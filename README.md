# ags-cavars

Visualizador de audio para tu barra de AGS. Las barras reaccionan a la música en tiempo real.

![Topbar completa](assets/topbar-full.png)

| Activo | Señal fuerte |
|:---:|:---:|
| ![](assets/cava-active.png) | ![](assets/cava-silent.png) |

---

## Instalación

Necesitas tener [Rust](https://rustup.rs) y [CAVA](https://github.com/karlstav/cava) (`sudo pacman -S cava`) instalados.

```bash
git clone https://github.com/tu-usuario/ags-cavars
cd ags-cavars
./install.sh
```

El script compila el binario, lo copia a `~/.local/bin` y deja el config de CAVA en `~/.config/cava/ags.ini`.

---

## Añadirlo a AGS

En tu `Right.tsx` (o donde tengas la barra):

```tsx
const [cava, setCava] = createState(
  "<span color='#8a8ea8'>▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁</span>",
)
subprocess(
  ["bash", "-c", `cava -p ${HOME}/.config/cava/ags.ini | ${HOME}/.local/bin/ags-cavars --ags --led`],
  (line) => setCava(line),
)
```

Y el widget:

```tsx
<box class="cava-pill" valign={Gtk.Align.CENTER} width-request={212} height-request={28}>
  <label class="cava-led-label" use-markup label={cava}
    width-chars={32} halign={Gtk.Align.FILL} valign={Gtk.Align.CENTER} />
</box>
```

CSS:

```css
.cava-pill {
    min-width: 200px;
    padding: 4px 10px;
}

.cava-led-label {
    font-family: "JetBrainsMono Nerd Font", monospace;
    font-size: 11px;
}
```

---

## Opciones

| Flag | Qué hace |
|---|---|
| `--ags` | Salida de texto para AGS (necesario) |
| `--led` | Colores VU meter: teal → dorado → coral → rosa |
| _(sin --led)_ | Usa los colores de pywal automáticamente |

---

## Algo no funciona

```bash
# Prueba el pipeline directamente
cava -p ~/.config/cava/ags.ini | ags-cavars --ags --led | head -5
```

Si ves líneas con `<span ...>`, funciona. Recarga AGS con `ags quit && ags run &`.

---

<sub>Arch Linux · Hyprland · AGS v2 · CAVA 0.10 · PipeWire</sub>
