use serde::Serialize;
use std::io::{self, Write};

use crate::colorizer::{self, ColorMode, SpecialState};

// ─── Estructura de salida ──────────────────────────────────────────────────────

/// Payload JSON que Waybar consume por stdout.
///
/// Waybar lee una línea JSON por frame cuando el módulo usa `return-type = json`.
/// Los campos opcionales se omiten si son `None` para mantener el output limpio.
#[derive(Serialize)]
pub struct WaybarOutput {
    /// Texto principal del módulo. Puede contener Pango Markup.
    pub text: String,

    /// Texto del tooltip al pasar el ratón. Soporta Pango Markup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,

    /// Clase CSS aplicada al widget — útil para colorear el fondo desde GTK.
    /// Ejemplo: "normal", "peak", "muted".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,

    /// Porcentaje numérico expuesto a Waybar (0–100).
    /// Permite usar el campo como fuente para barras de progreso externas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u8>,
}

impl WaybarOutput {
    /// Crea una salida mínima solo con `text`.
    pub fn text_only(text: impl Into<String>) -> Self {
        Self {
            text:       text.into(),
            tooltip:    None,
            class:      None,
            percentage: None,
        }
    }
}

// ─── Estado del visualizador ──────────────────────────────────────────────────

/// Estado interno del pipeline, actualizado cada frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisualizerState {
    /// Señal activa — emite barras coloreadas.
    Active,
    /// Sin señal durante varios frames seguidos.
    Silent,
    /// Audio muteado detectado externamente (futuro: integración con PipeWire).
    Muted,
}

/// Gestiona el estado del visualizador y genera el `WaybarOutput` por frame.
pub struct WaybarEmitter {
    color_mode:      ColorMode,
    silent_frames:   u32,
    /// Cuántos frames de silencio consecutivos activan el estado `Silent`.
    silent_threshold: u32,
    state:           VisualizerState,
}

impl WaybarEmitter {
    /// Crea un emisor con el modo de color deseado.
    ///
    /// `silent_threshold`: frames consecutivos de silencio antes de cambiar estado.
    /// Un valor de 30 a ~60fps equivale a ~0.5–1 segundo de silencio.
    pub fn new(color_mode: ColorMode, silent_threshold: u32) -> Self {
        Self {
            color_mode,
            silent_frames: 0,
            silent_threshold,
            state: VisualizerState::Active,
        }
    }

    /// Genera el `WaybarOutput` para el frame actual.
    ///
    /// `frame_data` es la salida de `mapper::build_frame_data`.
    /// `peak` es la amplitud máxima del frame (de `CavaFrame::peak()`).
    /// `is_silent` indica si el frame está por debajo del umbral de silencio.
    pub fn emit(
        &mut self,
        frame_data: &[(char, f32)],
        peak: f32,
        is_silent: bool,
    ) -> WaybarOutput {
        self.update_state(is_silent);

        match self.state {
            VisualizerState::Muted => WaybarOutput {
                text:       colorizer::state_markup(SpecialState::Muted).to_string(),
                tooltip:    Some("Audio muteado".into()),
                class:      Some("muted".into()),
                percentage: Some(0),
            },

            VisualizerState::Silent => WaybarOutput {
                text:       colorizer::state_markup(SpecialState::Standby).to_string(),
                tooltip:    None,
                class:      Some("silent".into()),
                percentage: Some(0),
            },

            VisualizerState::Active => {
                let text = colorizer::build_pango_frame(frame_data, self.color_mode);
                let pct  = (peak * 100.0).round() as u8;

                WaybarOutput {
                    text,
                    tooltip:    None,
                    class:      Some(css_class_for_peak(peak).into()),
                    percentage: Some(pct),
                }
            }
        }
    }

    /// Señala al emisor que CAVA no está disponible.
    pub fn emit_error(&self) -> WaybarOutput {
        WaybarOutput {
            text:       colorizer::state_markup(SpecialState::Error).to_string(),
            tooltip:    Some("CAVA no disponible".into()),
            class:      Some("error".into()),
            percentage: None,
        }
    }

    /// Actualiza `state` y el contador de frames silenciosos.
    fn update_state(&mut self, is_silent: bool) {
        if self.state == VisualizerState::Muted {
            return; // mute se activa/desactiva externamente
        }

        if is_silent {
            self.silent_frames += 1;
            if self.silent_frames >= self.silent_threshold {
                self.state = VisualizerState::Silent;
            }
        } else {
            self.silent_frames = 0;
            self.state = VisualizerState::Active;
        }
    }

    /// Activa o desactiva el estado de mute manualmente.
    pub fn set_muted(&mut self, muted: bool) {
        self.state = if muted {
            VisualizerState::Muted
        } else {
            VisualizerState::Active
        };
    }

    pub fn state(&self) -> VisualizerState {
        self.state
    }
}

/// Clase CSS según el nivel de pico — permite estilizar el fondo desde Waybar CSS.
fn css_class_for_peak(peak: f32) -> &'static str {
    match peak {
        p if p < 0.33 => "low",
        p if p < 0.66 => "mid",
        p if p < 0.85 => "high",
        _             => "peak",
    }
}

// ─── Escritor JSON ────────────────────────────────────────────────────────────

/// Serializa y escribe un `WaybarOutput` en el writer dado (normalmente stdout).
///
/// Waybar espera una línea JSON por frame, terminada en `\n`.
/// El `BufWriter` externo debe hacer `flush()` después de cada línea.
pub fn write_output<W: Write>(writer: &mut W, output: &WaybarOutput) -> io::Result<()> {
    let json = serde_json::to_string(output)
        .expect("serialización WaybarOutput no debería fallar");
    writeln!(writer, "{json}")
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colorizer::ColorMode;

    fn dummy_frame() -> Vec<(char, f32)> {
        vec![('▄', 0.5), ('▆', 0.7), ('█', 1.0), ('▂', 0.2)]
    }

    #[test]
    fn active_genera_spans() {
        let mut emitter = WaybarEmitter::new(ColorMode::ByAmplitude, 30);
        let out = emitter.emit(&dummy_frame(), 1.0, false);
        assert!(out.text.contains("<span"));
        assert_eq!(out.class.as_deref(), Some("peak"));
        assert_eq!(out.percentage, Some(100));
    }

    #[test]
    fn silencio_sostenido_cambia_estado() {
        let mut emitter = WaybarEmitter::new(ColorMode::ByAmplitude, 5);
        for _ in 0..5 {
            emitter.emit(&dummy_frame(), 0.0, true);
        }
        assert_eq!(emitter.state(), VisualizerState::Silent);
    }

    #[test]
    fn senyal_activa_tras_silencio() {
        let mut emitter = WaybarEmitter::new(ColorMode::ByAmplitude, 5);
        for _ in 0..5 {
            emitter.emit(&dummy_frame(), 0.0, true);
        }
        emitter.emit(&dummy_frame(), 0.8, false);
        assert_eq!(emitter.state(), VisualizerState::Active);
    }

    #[test]
    fn mute_produce_markup_especial() {
        let mut emitter = WaybarEmitter::new(ColorMode::ByAmplitude, 30);
        emitter.set_muted(true);
        let out = emitter.emit(&dummy_frame(), 0.5, false);
        assert_eq!(out.class.as_deref(), Some("muted"));
        assert_eq!(out.percentage, Some(0));
    }

    #[test]
    fn error_produce_clase_error() {
        let emitter = WaybarEmitter::new(ColorMode::ByAmplitude, 30);
        let out = emitter.emit_error();
        assert_eq!(out.class.as_deref(), Some("error"));
    }

    #[test]
    fn css_class_niveles() {
        assert_eq!(css_class_for_peak(0.1),  "low");
        assert_eq!(css_class_for_peak(0.5),  "mid");
        assert_eq!(css_class_for_peak(0.75), "high");
        assert_eq!(css_class_for_peak(0.95), "peak");
    }

    #[test]
    fn write_output_serializa_json() {
        let out = WaybarOutput::text_only("test");
        let mut buf = Vec::new();
        write_output(&mut buf, &out).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains(r#""text":"test""#));
        assert!(s.ends_with('\n'));
    }
}