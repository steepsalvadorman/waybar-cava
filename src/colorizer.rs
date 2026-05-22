use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Modo de degradado de color aplicado a las barras.
///
/// Cada modo define cómo se asigna el color a cada canal/glifo.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum ColorMode {
    /// Color por amplitud individual del canal.
    /// Usa la paleta del wallpaper cuando está disponible.
    ByAmplitude,

    /// Degradado horizontal fijo: el color depende de la posición del canal
    /// en el espectro (graves → agudos), independiente de la amplitud.
    ByPosition { total_channels: usize },

    /// Un solo color fijo para todo el frame. Útil para estados especiales
    /// (mute, standby) o preferencia estética minimalista.
    Flat { color: &'static str },
}

#[derive(Debug, Deserialize)]
struct WalColorsFile {
    colors: HashMap<String, String>,
}

const FALLBACK_WAL_PALETTE: &[&str] = &[
    "#00eaff",
    "#69ff47",
    "#ffe234",
    "#ffb300",
    "#ff8c00",
    "#ff4444",
    "#ff00aa",
    "#a259ff",
];

const WALLPAPER_COLORS_PATH: &str = ".cache/wal/colors.json";

/// Tabla de colores para `ByPosition` (espectro izquierda → derecha).
///
/// Se interpola por posición relativa [0.0, 1.0] del canal en el array.
const POS_RAMP: &[(&str, &str)] = &[
    ("0.0", "#7b68ee"),
    ("0.33", "#00bfff"),
    ("0.66", "#00fa9a"),
    ("1.0", "#ff6347"),
];

static WALLPAPER_PALETTE: OnceLock<Vec<String>> = OnceLock::new();

fn wallpaper_palette_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(WALLPAPER_COLORS_PATH)
}

pub fn load_palette_from_path(path: &Path) -> Option<Vec<String>> {
    let raw = fs::read_to_string(path).ok()?;
    let parsed: WalColorsFile = serde_json::from_str(&raw).ok()?;

    let keys = [
        "color1", "color2", "color3", "color4", "color5", "color6", "color7", "color8",
    ];

    let palette = keys
        .into_iter()
        .filter_map(|key| parsed.colors.get(key).cloned())
        .collect::<Vec<_>>();

    if palette.is_empty() {
        None
    } else {
        Some(palette)
    }
}

fn current_palette() -> &'static [String] {
    WALLPAPER_PALETTE.get_or_init(|| {
        load_palette_from_path(&wallpaper_palette_path())
            .unwrap_or_else(|| FALLBACK_WAL_PALETTE.iter().map(|color| (*color).to_string()).collect())
    })
}

fn color_for_amplitude_from_palette(amp: f32, palette: &[String]) -> String {
    if palette.is_empty() {
        return "#ff4444".to_string();
    }

    let clamped = amp.clamp(0.0, 1.0);
    let index = ((clamped * (palette.len() - 1) as f32).round() as usize).min(palette.len() - 1);
    palette[index].clone()
}

fn color_by_amplitude(amp: f32) -> String {
    color_for_amplitude_from_palette(amp, current_palette())
}

fn color_by_position(pos: f32) -> String {
    let pos = pos.clamp(0.0, 1.0);
    let next = POS_RAMP
        .iter()
        .find(|(threshold, _)| pos <= threshold.parse::<f32>().unwrap_or(1.0))
        .map(|(_, color)| *color)
        .unwrap_or(POS_RAMP.last().unwrap().1);

    next.to_string()
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
    let mut out = String::with_capacity(frame_data.len() * 16);

    let total = frame_data.iter().filter(|(c, _)| *c != ' ').count();
    let mut channel_idx: usize = 0;

    let mut current: Option<(String, bool, String)> = None;

    let flush_current = |out: &mut String, current: &mut Option<(String, bool, String)>| {
        if let Some((color, alpha, text)) = current.take() {
            out.push_str("<span color='");
            out.push_str(&color);
            if alpha {
                out.push_str("' alpha='40%'>");
            } else {
                out.push_str("'>");
            }
            out.push_str(&text);
            out.push_str("</span>");
        }
    };

    for &(glyph, amp) in frame_data {
        if glyph == ' ' && amp == 0.0 {
            flush_current(&mut out, &mut current);
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
            ColorMode::Flat { color } => color.to_string(),
        };

        let alpha = amp < 0.05;

        match current.as_mut() {
            Some((current_color, current_alpha, text)) if *current_color == color && *current_alpha == alpha => {
                text.push(glyph);
            }
            _ => {
                flush_current(&mut out, &mut current);
                current = Some((color, alpha, glyph.to_string()));
            }
        }

        channel_idx += 1;
        let _ = total;
    }

    flush_current(&mut out, &mut current);

    out
}

pub fn state_markup(state: SpecialState) -> &'static str {
    match state {
        SpecialState::Muted => "<span color='#888888'>󰝟</span>",
        SpecialState::Standby => "<span color='#555555'>▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁</span>",
        SpecialState::Error => "<span color='#ff4444'>⚠ cava</span>",
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SpecialState {
    Muted,
    Standby,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_amplitud_limites() {
        let fallback = FALLBACK_WAL_PALETTE.iter().map(|color| (*color).to_string()).collect::<Vec<_>>();
        assert_eq!(color_for_amplitude_from_palette(0.0, &fallback), "#00eaff");
        assert_eq!(color_for_amplitude_from_palette(1.0, &fallback), "#a259ff");
    }

    #[test]
    fn color_posicion_extremos() {
        let izq = color_by_position(0.0);
        let der = color_by_position(1.0);
        assert_ne!(izq, der, "extremos deben tener colores distintos");
    }

    #[test]
    fn color_por_amplitud_usa_la_paleta_proporcionada() {
        let palette = vec!["#111111".to_string(), "#222222".to_string(), "#333333".to_string()];
        assert_eq!(color_for_amplitude_from_palette(0.0, &palette), "#111111");
        assert_eq!(color_for_amplitude_from_palette(1.0, &palette), "#333333");
    }

    #[test]
    fn carga_paleta_de_wallpaper_desde_json() {
        let dir = std::env::temp_dir().join(format!("waybar-cavars-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("colors.json");
        std::fs::write(
            &file,
            "{\"colors\":{\"color1\":\"#aa0000\",\"color2\":\"#00aa00\",\"color3\":\"#0000aa\"}}",
        )
        .unwrap();

        let palette = load_palette_from_path(&file).unwrap();
        assert_eq!(palette, vec!["#aa0000", "#00aa00", "#0000aa"]);

        std::fs::remove_dir_all(&dir).unwrap();
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

        assert!(!markup.contains("#69ff47"));
    }

    #[test]
    fn state_markup_no_vacio() {
        assert!(!state_markup(SpecialState::Muted).is_empty());
        assert!(!state_markup(SpecialState::Standby).is_empty());
        assert!(!state_markup(SpecialState::Error).is_empty());
    }
}
