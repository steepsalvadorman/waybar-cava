/// Glifos de barra ordenados de menor a mayor amplitud.
///
/// Doble bloque para más impacto visual y altura.
const BAR_GLYPHS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '█'];

/// Glifos alternativos más "retro" (ASCII puro + llenos).
/// Descomenta y asigna a `BAR_GLYPHS` si prefieres este estilo.
// const BAR_GLYPHS_ASCII: &[char] = &[' ', '.', ':', '|', '|', 'I', 'I', 'I', 'I'];

/// Convierte una amplitud normalizada [0.0, 1.0] al glifo correspondiente.
///
/// La amplitud se mapea linealmente sobre el índice del array `BAR_GLYPHS`.
/// Valores fuera de rango se clampean silenciosamente.
///
/// # Ejemplo
/// ```
/// assert_eq!(amp_to_glyph(0.0), ' ');
/// assert_eq!(amp_to_glyph(1.0), '█');
/// ```
pub fn amp_to_glyph(amp: f32) -> char {
    let n = BAR_GLYPHS.len();
    let idx = (amp.clamp(0.0, 1.0) * (n - 1) as f32).round() as usize;
    BAR_GLYPHS[idx.min(n - 1)]
}

/// Genera el frame de texto crudo (sin color) para todos los canales.
///
/// Útil para debug o para separar la capa de mapeo de la de color.
pub fn build_raw_frame(values: &[f32]) -> String {
    values.iter().map(|&a| amp_to_glyph(a)).collect()
}

/// Modo de disposición de las barras en el frame final.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarLayout {
    /// Barras separadas por un espacio — más legible a fuentes anchas.
    Spaced,
    /// Barras contiguas — más compacto, mejor para fuentes monospace finas.
    Compact,
}

/// Construye el frame de texto aplicando el layout seleccionado.
///
/// Devuelve un `Vec<(char, f32)>` con el glifo y la amplitud original
/// para que `colorizer` pueda asignar color por canal sin recalcular.
pub fn build_frame_data(values: &[f32], layout: BarLayout) -> Vec<(char, f32)> {
    let mut out = Vec::with_capacity(values.len() * 2);

    for (i, &amp) in values.iter().enumerate() {
        // Doble bloque para más ancho
        let glyph = amp_to_glyph(amp);
        out.push((glyph, amp));
        out.push((glyph, amp));

        if layout == BarLayout::Spaced && i < values.len() - 1 {
            // Espacio separador con amplitud 0 (sin color)
            out.push((' ', 0.0));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limites() {
        assert_eq!(amp_to_glyph(0.0), ' ');
        assert_eq!(amp_to_glyph(1.0), '█');
    }

    #[test]
    fn clampeo() {
        assert_eq!(amp_to_glyph(-1.0), amp_to_glyph(0.0));
        assert_eq!(amp_to_glyph(2.0), amp_to_glyph(1.0));
    }

    #[test]
    fn mitad() {
        // 0.5 debe caer en el glifo central (~'▄')
        let g = amp_to_glyph(0.5);
        assert!(BAR_GLYPHS.contains(&g), "glifo inesperado: {g}");
    }

    #[test]
    fn build_raw_frame_longitud() {
        let v = vec![0.1, 0.5, 0.9];
        let frame = build_raw_frame(&v);
        assert_eq!(frame.chars().count(), 3);
    }

    #[test]
    fn layout_spaced_inserta_espacios() {
        let v = vec![0.5, 0.5];
        let data = build_frame_data(&v, BarLayout::Spaced);
        // Cada canal se duplica para ganar ancho visual y un separador agrega una entrada extra.
        assert_eq!(data.len(), 5);
        assert_eq!(data[2].0, ' ');
    }

    #[test]
    fn layout_compact_sin_espacios() {
        let v = vec![0.5, 0.5];
        let data = build_frame_data(&v, BarLayout::Compact);
        // Cada canal se duplica para ganar ancho visual incluso en modo compacto.
        assert_eq!(data.len(), 4);
    }
}