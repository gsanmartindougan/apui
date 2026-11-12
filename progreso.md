# Progreso

## Estado

Primera versión implementada. La app inicia una ventana de 860 × 610 px y reúne peticiones REST, conexión WebSocket y colecciones locales por carpetas.

## Implementado

- REST con métodos GET, POST, PUT, PATCH, DELETE, HEAD y OPTIONS; URL, parámetros, cabeceras, body, estado, tiempo, respuesta y errores.
- WebSocket `ws`/`wss`, cabeceras, conexión/desconexión, envío y recepción de mensajes, estado y errores.
- Guardado atómico de carpetas y rutas, edición al volver a guardar, movimiento entre carpetas y orden con arrastrar y soltar o botones arriba/abajo.
- Tema `#6b2e63` y `#1b3644`; diseño editable e historial preservados en FigCraft.
- Manual y especificación en `README.md` y `design/`.
- Params editables como tabla con enabled, Name, Value, Type y Description; las filas habilitadas generan la query URL. Rutas antiguas `params` como texto se convierten al cargar y mantienen una cadena compatible al guardar.
- Body con editor JSON resaltado y Beautify; texto inválido se conserva y muestra error.

## Verificación

- `cargo fmt --check`, `cargo check --offline` y `cargo build --offline` — correctos; binario en `target/debug/apui`.
- No se añadieron ni ejecutaron pruebas, según la instrucción del usuario.

## Distribución

- Empaquetado AppImage x86_64 mediante Docker con la imagen builder fijada y base compatible con glibc antigua.
- Script reproducible: `packaging/build-appimage.sh`; artefacto: `outputs/Apui-0.1.0-x86_64.AppImage`.

## Límites conocidos

- El visor HTTP limita cuerpos a 2 MiB.
- El registro WebSocket conserva los últimos 100 mensajes de la conexión actual.
- El editor usa un lexer pequeño para resaltar cadenas, claves, números, literales y puntuación JSON. El highlighter opcional de Iced requería una dependencia que no estaba disponible en la caché offline.
