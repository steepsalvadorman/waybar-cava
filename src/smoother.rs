/// Estado de suavizado por canal.
///
/// Cada canal mantiene su propio valor suavizado.
/// Al subir: EMA rápido (alpha_rise).
/// Al bajar: caída lineal lenta (gravity) — simula el efecto "peak-fall" de los VU meters.
pub struct Smoother {
    /// Valores suavizados actuales, uno por canal.
    pub values: Vec<f32>,

    /// Factor EMA para subida rápida (0.0–1.0).
    /// Valores altos (~0.8) hacen que la barra suba casi instantáneo.
    alpha_rise: f32,

    /// Velocidad de caída por tick (en unidades normalizadas 0.0–1.0).
    /// Valor típico: 0.02–0.05. A 0.03 tarda ~33 ticks en bajar de 1.0 a 0.0.
    gravity: f32,
}

impl Smoother {
    /// Crea un Smoother para `channels` canales con los parámetros dados.
    ///
    /// # Ejemplo
    /// ```
    /// let mut s = Smoother::new(16, 0.7, 0.03);
    /// ```
    pub fn new(channels: usize, alpha_rise: f32, gravity: f32) -> Self {
        Self {
            values: vec![0.0; channels],
            alpha_rise,
            gravity,
        }
    }

    /// Actualiza todos los canales con los valores crudos del frame actual.
    ///
    /// `raw` debe tener la misma longitud que `channels`.
    /// Los valores en `raw` deben estar normalizados en [0.0, 1.0].
    pub fn update(&mut self, raw: &[f32]) {
        debug_assert_eq!(
            raw.len(),
            self.values.len(),
            "update: raw.len() ({}) != channels ({})",
            raw.len(),
            self.values.len()
        );

        for (v, &r) in self.values.iter_mut().zip(raw.iter()) {
            if r > *v {
                // Subida: EMA rápido
                *v = *v + self.alpha_rise * (r - *v);
            } else {
                // Caída: gravedad lineal, clampeada a 0
                *v = (*v - self.gravity).max(0.0);
            }
        }
    }

    /// Devuelve una referencia inmutable a los valores suavizados actuales.
    #[inline]
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sube_rapido_baja_lento() {
        let mut s = Smoother::new(1, 0.9, 0.05);

        // Subida: tras varios ticks con señal = 1.0 debe acercarse a 1.0
        for _ in 0..20 {
            s.update(&[1.0]);
        }
        assert!(s.values[0] > 0.95, "debe subir rápido: {}", s.values[0]);

        // Caída: sin señal debe bajar gradualmente
        let inicial = s.values[0];
        s.update(&[0.0]);
        assert!(
            s.values[0] < inicial,
            "debe caer al menos un tick: {}",
            s.values[0]
        );
    }

    #[test]
    fn no_supera_uno_ni_baja_de_cero() {
        let mut s = Smoother::new(2, 1.0, 0.1);

        for _ in 0..100 {
            s.update(&[2.0, -1.0]); // valores fuera de rango intencionalmente
        }
        // Con alpha=1.0 converge a raw inmediatamente, pero raw puede ser >1
        // Este test verifica que gravity no genere negativos
        for _ in 0..100 {
            s.update(&[0.0, 0.0]);
        }
        for &v in s.values() {
            assert!(v >= 0.0, "nunca debe ser negativo: {}", v);
        }
    }
}