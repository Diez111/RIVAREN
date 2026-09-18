# ADR-008: Pulso (sistema de señales tipo redstone, original)

- Fecha: 2026-09-18. Estado: aceptado.
- Componentes: cable (pérdida 1), antorcha (inversor), palanca, botón (20 ticks),
  compuerta Y/O/NO/XOR, lámpara (carga) y pistón simplificado.
- Modelo: mapa de nodos + cola de ticks programados (BinaryHeap), límite de
  actualizaciones por tick (256) y propagación solo ante cambios de estado.
- Las cargas (lámpara/pistón) NO realimentan la red (evita oscilación infinita).
- Los cambios de estado se aplican al mundo por intercambio de bloque (p. ej.
  lámpara 86 apagada ↔ 87 encendida) y re-mesh con presupuesto de 2 chunks/frame.
