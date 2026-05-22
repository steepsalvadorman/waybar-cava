/// Modo de degradado de color aplicado a las barras.
///
/// Cada modo define cómo se asigna el color a cada canal/glifo.
#[derive(Debug, Clone, Copy)]
pub enum ColorMode {
    /// Color por amplitud individual del canal.
    /// Verde → Amarillo → Naranja → Rojo según qué tan alto esté cada barra.
    ByAmplitude,

    /// Degradado horizontal fijo: el color depende de la posición del canal
    /// en el espectro (graves → agudos), independiente de la amplitud.
    ByPosition { total_channels: usize },

    /// Un solo color fijo para todo el frame. Útil para estados especiales
    /// (mute, standby) o preferencia estética minimalista.
    Flat { color: &'static str },
}

/// Tabla de colores para `ByAmplitude`.
///
/// Cada entrada es un umbral de amplitud y el color Pango hex asociado.
/// Los umbrales deben estar en orden ascendente.
const AMP_RAMP: &[(f32, &str)] = &[
    (0.33, "#69ff47"), // bajo   → verde
    (0.60, "#ffe234"), // medio  → amarillo
    (0.80, "#ff8c00"), // alto   → naranja
    (1.01, "#ff4444"), // pico   → rojo
];

/// Tabla de colores para `ByPosition` (espectro izquierda → derecha).
///
/// Se interpola por posición relativa [0.0, 1.0] del canal en el array.
const POS_RAMP: &[(f32, &str)] = &[
    (0.0,  "#7b68ee"), // graves → violeta
    (0.33, "#00bfff"), // medios → azul claro
    (0.66, "#00fa9a"), // medios-altos → verde menta
    (1.0,  "#ff6347"), // agudos → naranja-rojo
];

// ─── Lógica de color ───────────────────────────────────────────────────────────

/// Devuelve el color Pango para una amplitud dada usando `AMP_RAMP`.
fn color_by_amplitude(amp: f32) -> &'static str {
    AMP_RAMP
        .iter()
        .find(|(threshold, _)| amp <= *threshold)
        .map(|(_, color)| *color)
        .unwrap_or("#ff4444")
}

/// Devuelve el color Pango para una posición relativa [0.0, 1.0] usando `POS_RAMP`.
fn color_by_position(pos: f32) -> &'static str {
    // Busca el segmento entre dos paradas y devuelve la más cercana
    // (interpolación de color real requeriría parsear hex — esto es suficiente
    //  para Waybar y se puede extender si hace falta)
    POS_RAMP
        .windows(2)
        .find(|w| pos <= w[1].0)
        .map(|w| {
            // Devuelve la parada más cercana al valor de posición
            let mid = (w[0].0 + w[1].0) / 2.0;
            if pos < mid { w[0].1 } else { w[1].1 }
        })
        .unwrap_or(POS_RAMP.last().unwrap().1)
}

// ─── API pública ───────────────────────────────────────────────────────────────

/// Construye el string Pango Markup completo para un frame.
///
/// `frame_data` es la salida de `mapper::build_frame_data`:
/// una lista de `(glifo, amplitud)` donde los espacios separadores
/// tienen amplitud 0.0 y se emiten sin etiqueta de color.
///
/// # Ejemplo
/// ```
/// let data = vec![('▄', 0.5), ('█', 1.0)];
/// let markup = build_pango_frame(&data, ColorMode::ByAmplitude);
/// assert!(markup.contains("<span"));
/// ```
pub fn build_pango_frame(frame_data: &[(char, f32)], mode: ColorMode) -> String {
    let mut out = String::with_capacity(frame_data.len() * 36);

    // Cuenta solo los canales reales (excluye espacios separadores)
    let total = frame_data.iter().filter(|(c, _)| *c != ' ').count();
    let mut channel_idx: usize = 0;

    for &(glyph, amp) in frame_data {
        if glyph == ' ' && amp == 0.0 {
            // Espacio separador — sin markup
            out.push(' ');
            continue;
        }

        let color = match mode {
            ColorMode::ByAmplitude => color_by_amplitude(amp),

            ColorMode::ByPosition { total_channels } => {
                let n = total_channels.max(1);
                let pos = channel_idx as f32 / (n - 1).max(1) as f32;
                color_by_position(pos)
            }

            ColorMode::Flat { color } => color,
        };

        // Opacidad reducida para barras muy bajas — efecto "apagado"
        if amp < 0.05 {
            out.push_str(&format!("<span color='{color}' alpha='40%'>{glyph}</span>"));
        } else {
            out.push_str(&format!("<span color='{color}'>{glyph}</span>"));
        }

        channel_idx += 1;
        let _ = total; // usado solo para doc
    }

    out
}

/// Genera markup Pango para un estado especial (mute, error, standby).
///
/// Devuelve un string con un icono o texto coloreado listo para Waybar.
pub fn state_markup(state: SpecialState) -> &'static str {
    match state {
        SpecialState::Muted   => "<span color='#888888'>󰝟</span>",
        SpecialState::Standby => "<span color='#555555'>▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁</span>",
        SpecialState::Error   => "<span color='#ff4444'>⚠ cava</span>",
    }
}

/// Estados especiales del visualizador.
#[derive(Debug, Clone, Copy)]
pub enum SpecialState {
    /// Audio muteado — muestra icono de silencio.
    Muted,
    /// Sin señal activa — barras planas en gris.
    Standby,
    /// CAVA no disponible o pipe roto.
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_amplitud_limites() {
        assert_eq!(color_by_amplitude(0.0), "#69ff47");
        assert_eq!(color_by_amplitude(1.0), "#ff4444");
    }

    #[test]
    fn color_posicion_extremos() {
        let izq = color_by_position(0.0);
        let der = color_by_position(1.0);
        assert_ne!(izq, der, "extremos deben tener colores distintos");
    }

    #[test]
    fn pango_frame_contiene_spans() {
        let data = vec![('▄', 0.5), ('█', 1.0)];
        let markup = build_pango_frame(&data, ColorMode::ByAmplitude);
        assert!(markup.contains("<span"));
        assert!(markup.contains("color="));
    }

    #[test]
    fn pango_frame_espacio_sin_markup() {
        // Espacio separador no debe generar <span>
        let data = vec![('▄', 0.5), (' ', 0.0), ('▄', 0.5)];
        let markup = build_pango_frame(&data, ColorMode::ByAmplitude);
        let span_count = markup.matches("<span").count();
        assert_eq!(span_count, 2, "solo 2 spans, el espacio va sin markup");
    }

    #[test]
    fn barra_baja_tiene_alpha() {
        let data = vec![('▁', 0.01)];
        let markup = build_pango_frame(&data, ColorMode::ByAmplitude);
        assert!(markup.contains("alpha="), "barras casi vacías deben tener alpha reducido");
    }

    #[test]
    fn flat_mode_un_solo_color() {
        let data = vec![('▄', 0.2), ('▆', 0.7)];
        let markup = build_pango_frame(&data, ColorMode::Flat { color: "#ffffff" });
        assert!(markup.contains("#ffffff"));
        // No debe aparecer ningún otro color
        assert!(!markup.contains("#69ff47"));
    }

    #[test]
    fn state_markup_no_vacio() {
        assert!(!state_markup(SpecialState::Muted).is_empty());
        assert!(!state_markup(SpecialState::Standby).is_empty());
        assert!(!state_markup(SpecialState::Error).is_empty());
    }
}