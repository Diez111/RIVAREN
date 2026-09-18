# ADR-002: Jobs <1ms con deadlines + fixed 20 TPS

- Ningún sistema bloquea el frame; todo job chequea AtomicBool cada N iters.
- CPU budget 40% / GPU 60% del frame; resolución dinámica 0.5-1.0.
