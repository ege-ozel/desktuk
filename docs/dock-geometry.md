# Compact panel dimensions / Küçük panel ölçüleri

For matching VoiceType's compact width to DeskTuk, use **64 DIP**. The compact
DeskTuk window is **64 × 132 DIP** (width × height), including its painted border.
The window shadow is outside this rectangle. The expanded panel is 168 DIP wide.

VoiceType'ın küçük panel genişliğini DeskTuk ile eşlemek için **64 DIP** kullanın.
Küçük DeskTuk penceresi, çizilen kenarlık dahil **64 × 132 DIP** ölçüsündedir
(genişlik × yükseklik). Pencere gölgesi bu ölçüye dahil değildir. Açık panelin
genişliği 168 DIP'tir.

| Windows scale / Ölçek | DPI | Width / Genişlik (px) | Height / Yükseklik (px) |
| --- | --- | --- | --- |
| 100% | 96 | 64 | 132 |
| 125% | 120 | 80 | 165 |
| 150% | 144 | 96 | 198 |
| 175% | 168 | 112 | 231 |
| 200% | 192 | 128 | 264 |

Use the monitor's DPI: `pixels = (DIP * DPI + 48) / 96` with integer division.
These are window dimensions, not the 48 × 48 DIP visibility button inside it.
