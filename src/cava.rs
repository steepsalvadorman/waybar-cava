use std::io::{self, Read};

// ─── Configuración del stream ──────────────────────────────────────────────────

/// Formato de datos que CAVA puede emitir en modo `raw`.
#[derive(Debug, Clone, Copy)]
pub enum CavaFormat {
    /// i16 little-endian por canal — el más común y eficiente.
    /// Requiere `data_format = binary` en cava/config.
    BinaryI16,

    /// f32 little-endian por canal.
    /// Requiere `data_format = binary_float` en cava/config.
    BinaryF32,
}

/// Configuración del lector de CAVA.
#[derive(Debug, Clone)]
pub struct CavaConfig {
    /// Número de canales (barras). Debe coincidir con `bars` en cava/config.
    pub channels: usize,

    /// Formato de datos del stream.
    pub format: CavaFormat,

    /// Valor máximo esperado para normalización (solo relevante en BinaryI16).
    /// CAVA usa i16::MAX = 32767 por defecto.
    pub max_value: f32,
}

impl CavaConfig {
    /// Crea una configuración estándar para el uso más común:
    /// 16 canales, i16 binario, rango 0–32767.
    pub fn default_binary(channels: usize) -> Self {
        Self {
            channels,
            format: CavaFormat::BinaryI16,
            max_value: i16::MAX as f32,
        }
    }

    /// Bytes que ocupa un frame completo según el formato.
    pub fn frame_bytes(&self) -> usize {
        let bytes_per_sample = match self.format {
            CavaFormat::BinaryI16 => 2,
            CavaFormat::BinaryF32 => 4,
        };
        self.channels * bytes_per_sample
    }
}

// ─── Lector de frames ──────────────────────────────────────────────────────────

/// Lee y parsea frames del stream binario de CAVA.
///
/// Mantiene un buffer interno reutilizable para evitar allocations por frame.
pub struct CavaReader<R: Read> {
    inner:  R,
    config: CavaConfig,
    buf:    Vec<u8>,
}

impl<R: Read> CavaReader<R> {
    /// Crea un lector a partir de cualquier `Read` (stdin, named pipe, archivo de prueba).
    pub fn new(reader: R, config: CavaConfig) -> Self {
        let buf = vec![0u8; config.frame_bytes()];
        Self { inner: reader, config, buf }
    }

    /// Lee el siguiente frame del stream.
    ///
    /// Devuelve:
    /// - `Ok(Some(frame))` — frame leído y normalizado a [0.0, 1.0]
    /// - `Ok(None)`        — EOF, CAVA cerró el pipe limpiamente
    /// - `Err(_)`          — error de I/O inesperado
    pub fn next_frame(&mut self) -> io::Result<Option<CavaFrame>> {
        if !self.read_exact_buf()? {
            return Ok(None);
        }

        let values = self.parse_buf();
        Ok(Some(CavaFrame { values }))
    }

    /// Lee exactamente `frame_bytes` bytes en el buffer interno.
    /// Maneja lecturas parciales e interrupciones.
    fn read_exact_buf(&mut self) -> io::Result<bool> {
        let n = self.buf.len();
        let mut total = 0;

        while total < n {
            match self.inner.read(&mut self.buf[total..]) {
                Ok(0)  => return Ok(false), // EOF limpio
                Ok(k)  => total += k,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }

        Ok(true)
    }

    /// Decodifica el buffer interno según el formato configurado.
    fn parse_buf(&self) -> Vec<f32> {
        match self.config.format {
            CavaFormat::BinaryI16 => self
                .buf
                .chunks_exact(2)
                .map(|b| {
                    let raw = i16::from_le_bytes([b[0], b[1]]);
                    (raw as f32 / self.config.max_value).clamp(0.0, 1.0)
                })
                .collect(),

            CavaFormat::BinaryF32 => self
                .buf
                .chunks_exact(4)
                .map(|b| {
                    let raw = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    raw.clamp(0.0, 1.0)
                })
                .collect(),
        }
    }

    /// Referencia a la configuración activa.
    pub fn config(&self) -> &CavaConfig {
        &self.config
    }
}

// ─── Frame ────────────────────────────────────────────────────────────────────

/// Un frame de audio procesado: amplitudes normalizadas por canal.
#[derive(Debug, Clone)]
pub struct CavaFrame {
    /// Amplitudes normalizadas en [0.0, 1.0], una por canal.
    pub values: Vec<f32>,
}

impl CavaFrame {
    /// Indica si el frame está en silencio (todas las amplitudes bajo el umbral).
    pub fn is_silent(&self, threshold: f32) -> bool {
        self.values.iter().all(|&v| v < threshold)
    }

    /// Amplitud máxima del frame — útil para detección de picos.
    pub fn peak(&self) -> f32 {
        self.values.iter().cloned().fold(0.0_f32, f32::max)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn make_i16_frame(channels: usize, value: i16) -> Vec<u8> {
        (0..channels)
            .flat_map(|_| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn frame_bytes_i16() {
        let cfg = CavaConfig::default_binary(16);
        assert_eq!(cfg.frame_bytes(), 32);
    }

    #[test]
    fn frame_bytes_f32() {
        let cfg = CavaConfig {
            channels: 8,
            format: CavaFormat::BinaryF32,
            max_value: 1.0,
        };
        assert_eq!(cfg.frame_bytes(), 32);
    }

    #[test]
    fn lee_frame_maximo() {
        let data = make_i16_frame(4, i16::MAX);
        let cfg  = CavaConfig::default_binary(4);
        let mut reader = CavaReader::new(Cursor::new(data), cfg);

        let frame = reader.next_frame().unwrap().unwrap();
        assert_eq!(frame.values.len(), 4);
        for v in &frame.values {
            assert!((v - 1.0).abs() < 1e-4, "esperado ~1.0, got {v}");
        }
    }

    #[test]
    fn lee_frame_cero() {
        let data = make_i16_frame(4, 0);
        let cfg  = CavaConfig::default_binary(4);
        let mut reader = CavaReader::new(Cursor::new(data), cfg);

        let frame = reader.next_frame().unwrap().unwrap();
        for v in &frame.values {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn eof_devuelve_none() {
        let cfg = CavaConfig::default_binary(4);
        let mut reader = CavaReader::new(Cursor::new(vec![]), cfg);
        assert!(reader.next_frame().unwrap().is_none());
    }

    #[test]
    fn frame_parcial_no_falla() {
        // Solo 2 bytes cuando se esperan 8 — debe devolver None (EOF parcial)
        let data = vec![0u8; 2];
        let cfg  = CavaConfig::default_binary(4); // espera 8 bytes
        let mut reader = CavaReader::new(Cursor::new(data), cfg);
        // EOF en mitad de frame — Ok(None) porque read devuelve 0 tras agotar los bytes
        let result = reader.next_frame();
        assert!(result.is_ok());
    }

    #[test]
    fn is_silent() {
        let frame = CavaFrame { values: vec![0.01, 0.02, 0.0] };
        assert!(frame.is_silent(0.05));
        assert!(!frame.is_silent(0.01));
    }

    #[test]
    fn peak() {
        let frame = CavaFrame { values: vec![0.2, 0.8, 0.5] };
        assert!((frame.peak() - 0.8).abs() < 1e-6);
    }
}