# THE PIT

Experiência PCVR para Meta Quest 3: o participante está numa plataforma suspensa no meio de um poço vertical gigante e aparentemente infinito, com estética industrial de ficção científica de finais dos anos 70. O espaço reconfigura-se e passa por vários estados, do mecânico opressivo ao clínico high-tech.

Escrito em **Rust** com [Bevy](https://bevy.org) 0.19 e [bevy_mod_openxr](https://crates.io/crates/bevy_mod_openxr) 0.6. Não há editor: tudo é código.

![Protótipo de escala](docs/prototipo-escala.jpg)

## Estado atual: protótipo de escala

Blockout sem texturas para sentir a escala no óculos antes de investir em detalhe.

| Elemento | Medida |
|---|---|
| Poço | 12 faces, diâmetro 90 m (alternativas 60 m e 120 m) |
| Módulo | 30 m de altura, 4 variantes empilhadas ±600 m |
| Plataforma | Ø 12 m, chão a y = 0 |
| Corrimão | 1,1 m |
| Ponte | da plataforma até à parede (39 m no poço de 90 m) |
| Passerelles, portas, escadas | 2,1 m de porta, escada com degraus a 30 cm |
| Figuras de referência | 1,8 m, na plataforma, no fim da ponte, na passerelle oposta e em cima da viga do nível acima |

Os 4 estados de material e luz são parâmetros, por isso a transição mecânico ↔ clínico demora 4 s e não custa nada em performance.

## Correr no Quest 3

1. Descarrega `the-pit-windows` (o `.exe`) do último run de **Build Windows** no separador *Actions* do GitHub.
2. Liga o Quest 3 ao PC com **Meta Quest Link** (cabo ou Air Link) e confirma, na app Meta Quest Link do PC, que a Meta está definida como runtime OpenXR ativo (*Settings → General → OpenXR Runtime*).
3. Corre `the_pit.exe`. Ficas no centro da plataforma; o chão real corresponde ao chão da plataforma (espaço *stage*).

### Comandos

| Ação | Comando Touch | Teclado |
|---|---|---|
| Mecânico ↔ clínico | A | L |
| Mudar diâmetro do poço (60 / 90 / 120 m) | B | 1 / 2 / 3 |
| Anel a descer / parar (teste de enjoo) | X | R |
| Sair | | Esc |

### Sem óculos

`the_pit.exe --desktop` abre uma janela normal: botão direito do rato para olhar, WASD para andar, Q/E para descer/subir, Shift para ir depressa.

Para gerar uma captura: `the_pit --shot captura.png --view up|down|wide|ring|clinical`.

## Compilar

Precisa de Rust estável (no Windows, com as build tools do MSVC).

```
cargo run --release              # VR
cargo run --release -- --desktop # janela
```

## Texturas

As texturas finais vêm do [Fab](https://www.fab.com). Este protótipo ainda não usa nenhuma.
