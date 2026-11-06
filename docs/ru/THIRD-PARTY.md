# Компоненты и лицензии

[<kbd>English</kbd>](../../THIRD-PARTY.md)

| Компонент | Версия | Назначение |
|---|---|---|
| Qt / Qt Quick | 6.8.3 | Интерфейс Windows с динамической загрузкой библиотек |
| sing-box | 1.14.4 | Основное сетевое ядро |
| Xray | 26.10.31 | Дополнительные транспорты |
| Зависимости Rust | [Cargo.lock](../../backend/Cargo.lock) | Контроллер, хранилище и диагностика |
| Microsoft C++ Runtime | 14.x | Библиотеки среды выполнения |

Код NetPulse распространяется по GPL-3.0-or-later. Для Qt применяются условия
LGPLv3 или GPLv3. Библиотеки Qt можно заменить совместимой сборкой.
[Лицензия NetPulse](../../LICENSE.txt) · [Лицензии Qt](../../licenses/qt)

В комплекте используются официальные Windows-сборки sing-box и Xray. Архив
`NetPulse-Engine-Sources.zip` содержит соответствующие исходники ядер и
зависимостей. [Манифест исходников](../../resources/cores/source-manifest.json) ·
[Лицензии ядер](../../resources/cores/licenses)

Флаги взяты из flag-icons 7.5.0 под лицензией MIT. Названия стран на английском и
русском взяты из Unicode CLDR 48.0.0.
[Лицензия флагов](../../licenses/flag-icons.txt) ·
[Лицензия Unicode](../../licenses/unicode-cldr.txt)

Уведомления предыдущей версии сохранены в архиве лицензий.
[Архивное уведомление](../../licenses/NETPULSE-V2-NOTICE.md)
