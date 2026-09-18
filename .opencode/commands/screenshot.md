---
description: Genera capturas de UI y de juego para verificar cambios visuales.
agent: ui-dev
---
Ejecuta:
1. `cargo run -p rivaren-app --release -- --screenshot /tmp/rivaren_ui.ppm`
2. `cargo run -p rivaren-app --release -- --screenshot-ingame /tmp/rivaren_game.ppm`
Convierte con `magick` a PNG y describe qué se ve. Señala desbordes o colores raros.
