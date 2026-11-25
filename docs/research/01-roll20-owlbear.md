# 01 — Инвентаризация функций Roll20 и Owlbear Rodeo 2

**Задача:** task-1. **Дата сбора:** 25.11.2026.
**Метод:** официальная документация (help.roll20.net, roll20partners.zendesk.com, docs.owlbear.rodeo, extensions.owlbear.rodeo), блог разработчиков, страницы тарифов; вторичные источники (обзоры 2026) — только там, где официальных страниц получить не удалось.
**Ограничения доступа:** `wiki.roll20.net` отдаёт HTTP 403 (Cloudflare) для автоматических запросов, поэтому факты, известные только из вики Roll20, помечены как «не подтверждено официальной страницей». Блог-пост Roll20 о выводе мобильного приложения отдаётся пустым телом (JS-рендер) — проверено только существование и заголовок поста.

Обозначения: ✅ есть · ⚠️ частично / с оговорками · ❌ нет · 🧩 только расширением/скриптом · 💰 платно · 🆓 бесплатно.

---

# A. Roll20 — полный разбор

## A1. Модель распространения и тарифы

Roll20 — облачный браузерный VTT: хостинг на серверах Roll20, игроки подключаются по ссылке, установки не требуется. Офлайн-режима нет by design («Dedicated Online Hosting» — заявлено как базовое свойство всех тарифов). Источник: [Feature Breakdown](https://help.roll20.net/hc/en-us/articles/360037774633-Feature-Breakdown) (обновлено 17.11.2025).

| Тариф | Цена | Стейдж (загрузки) | Макс. файл | Шеринг контента | Ключевое |
|---|---|---|---|---|---|
| **Free** | $0 🆓 | 100 МБ | 20 МБ | 1 игра, до 5 игроков | Хостинг, 1200+ систем, LFG (2 объявления), 3 переноса персонажа |
| **Plus** | $5.99/мес или $59.99/год 💰 | 4 ГБ | 50 МБ (аудио 20 МБ) | 3 игры, до 10 игроков | **Dynamic Lighting**, внешний доступ к журналу, без рекламы на загрузке, Demiplane Unlimited |
| **Pro** | $10.99/мес или $109.99/год 💰 | 10 ГБ | 100 МБ | 5 игр, до 15 игроков | **Mod (API)**, Transmogrifier, кастомные листы персонажей, кастомные фоны игры, Early Access |
| **Elite** | $14.99/мес или $149.99/год 💰 | 50 ГБ | 100 МБ | 20 игр, до 15 игроков | Dungeon Scrawl Pro, «distinguished» LFG, бейдж |

Примечания:
- Кастомные листы персонажей (HTML/CSS) — только Pro и выше; правка community-листов тоже Pro ([Character Sheet Tips and Tricks](https://help.roll20.net/hc/en-us/articles/360037773373-Character-Sheet-Tips-and-Tricks)).
- Marketplace ([marketplace.roll20.net](https://marketplace.roll20.net)) — отдельная модель: покупка модулей приключений, наборов токенов, карт-аддонов, саундпаков. Подписка и покупки не связаны напрямую, но многие аддоны рассчитаны на Plus+ (например, готовый Dynamic Lighting в купленных картах — [Setting Up Dynamic Lighting](https://roll20partners.zendesk.com/hc/en-us/articles/10269638920599-Setting-Up-Dynamic-Lighting)).
- Тариф **Elite** — новый уровень (появился в актуальной версии Feature Breakdown, ноябрь 2025); в обзорах 2026 года его обычно ещё не упоминают.
- Цены — только подписка; готовый контент покупается отдельно, возвратов/офлайна нет.

## A2. Игровое поле и слои

Слои (снизу вверх) — [Layers](https://help.roll20.net/hc/en-us/articles/360039675053-Layers):

| Слой | Назначение | Доступ |
|---|---|---|
| **Map & Background** | Статичная карта, мебель, декор | все |
| **Objects & Tokens** | Токены, радиальное меню, бары, маркеры | все |
| **GM Info Overlay** | Заметки/рисунки только для ГМ; сюда же прячут токены | только ГМ |
| **Foreground** | Объекты над токенами (кроны деревьев, крыши, «исчезающие» при входе) | Plus/Pro |
| **Dynamic Lighting** | Линии/фигуры, блокирующие свет и обзор | Plus/Pro |

- Слоя «Weather» в Roll20 **нет**. Есть только эффекты тьмы: Overlay (чёрный), Dark Fog, Pale Mist ([Page Settings For Dynamic Lighting](https://help.roll20.net/hc/en-us/articles/360052521913-Page-Settings-For-Dynamic-Lighting)).
- Переключение слоёв: Ctrl+O (объекты), Ctrl+K (GM Info), Ctrl+M (карта), Ctrl+. (Foreground), Ctrl+, (Lighting).
- Сетки и измерения ([Page Settings](https://help.roll20.net/hc/en-us/articles/360039675373-Page-Settings)):
  - типы: **square, hex (H), hex (V)**;
  - способ измерения дистанции: **D&D 5e/4e, 3.5/Pathfinder, Euclidean, Manhattan, Hex Grid**;
  - базовая метрика: **70 px = 1 дюйм** при 100 %; размер клетки задаётся множителем или в пикселях;
  - Page Scale: 5 ft по умолчанию, единицы **ft / m / km / mi** + произвольная единица; отдельно — **Daggerheart Range Scale** (melee/close/far);
  - цвет/прозрачность линий, отключение сетки отключает и снап.
- Измерительные инструменты: линейка (Ruler) с настраиваемым способом измерения; «Only Update on Drop» снижает нагрузку на больших картах (официальная рекомендация).
- Холст страницы ограничен настраиваемым размером (кратно 70 px); страницы можно архивировать, но архивная страница всё равно грузится при старте игры (то есть архив ≠ экономия размера игры).

## A3. Токены

Источник: [Token Features](https://help.roll20.net/hc/en-us/articles/360039674573-Token-Features) (обновлено 02.11.2026).

| Свойство | Детали |
|---|---|
| Создание | любой образ на слое Objects или GM Info становится токеном |
| Масштаб/поворот | ручки по углам/сторонам + ручка поворота; Alt — отключить снап при ресайзе; Advanced → Set Dimensions |
| Бар-числа (bubbles) | 3 по умолчанию, 4 через «Maximum Token Bubbles/Bars»; значения могут быть числовыми, текстовыми, относительными («+5») |
| Бары (полосы) | каждый из 3 баров: видимость и редактируемость для игроков; два поля — текущее/максимум; привязка к атрибуту листа персонажа |
| Маркеры-статусы | 40+ маркеров + цветные точки, красный X; несколько одновременно; цифра 0–9 на маркере; позиция (top/bottom/left/right) глобально в настройках |
| Nameplate / Tooltip | имя на токене; tooltip до 150 символов (доступность, скринридеры) |
| GM Notes | приватный текст ГМ |
| Tint / Lock | цветовой фильтр; блокировка позиции; глобальные /pausemovement и /resumemovement |
| **Ауры** | до **2 аур** на токен; стили Circle, Square, Hex + «Border»-варианты; радиус, цвет, прозрачность через `#RRGGBBAA` |
| Represents Character | связь с листом: наследует права «Can Be Edited & Controlled By», бары могут тянуть атрибуты; Shift+двойной клик — Bio, Alt+двойной — лист |
| Controlled By | несколько игроков на токен, «All Players», ГМ всегда控制 |
| Default Token | правки «Edit Token Properties» и «Apply Token Defaults» для всех будущих/существующих токенов персонажа |
| Vision / Light | отдельная вкладка Dynamic Lighting (см. A4) |
| Token Actions | ⚠️ отдельной сущности «Token Actions» в актуальном Help Center нет; аналог — drag&drop кнопок листа на Macro Quick Bar + Abilities персонажа (вики-термин, не подтверждён страницей Help Center) |

## A4. Туман войны, стены, свет

В Roll20 сосуществуют **три** механизма:

| Механизм | Тариф | Как работает | Ограничения |
|---|---|---|---|
| **Fog of War** (в новом движке переименован в **Hide/Reveal Mask**) | все 🆓 | ручное раскрытие прямоугольником/полигоном, скрытие областей, Reset Fog; прячет всё, кроме собственных токенов игрока | полностью ручной; ГМ контролирует вручную ([Fog of War](https://help.roll20.net/hc/en-us/articles/360037774513-Fog-of-War-Classic-VTT)) |
| **Advanced Fog of War** | Plus/Pro 💰 | авто-раскрытие по пути токена, клетками сетки; у каждого токена своя «память»; сочетается с Legacy Lighting | только с Legacy Lighting; клеточная (рваные края у стен не по сетке); нет сброса для одного токена; кап **50 000 клеток**; заметно грузит игру ([Advanced Fog of War](https://help.roll20.net/hc/en-us/articles/360037774493-Advanced-Fog-of-War-Classic-VTT)) |
| **Dynamic Lighting (UDL)** | Plus/Pro 💰 | реальные линии обзора/света в реальном времени, стены, двери, окна | требует настройки каждого токена и страницы; производительность ([What Is Dynamic Lighting?](https://help.roll20.net/hc/en-us/articles/4403801475479-What-Is-Dynamic-Lighting)) |

Dynamic Lighting — детали ([Setting Up](https://roll20partners.zendesk.com/hc/en-us/articles/10269638920599-Setting-Up-Dynamic-Lighting), [Page Settings DL](https://help.roll20.net/hc/en-us/articles/360052521913-Page-Settings-For-Dynamic-Lighting), [Token Settings DL](https://help.roll20.net/hc/en-us/articles/360051754954-Token-Settings-for-Dynamic-Lighting)):
- Барьеры рисуются на слое Dynamic Lighting: **Walls** (блокируют с обеих сторон), **One Way** (свет проходит в одну сторону), **Transparent** (видно, но не пройти).
- **Двери**: цвет линии, Locked, Opened, **Secret**; **окна**: цвет, Locked, Opened.
- «Restrict Movement» / «Lines Block Movement» — токены не проходят сквозь барьеры (нужно для дверей/окон).
- Токен: **Vision** (вкл/выкл), **Night Vision** (дистанция, тинт, Sharpen, Dimming), **Nocturnal Vision** (эмуляция darkvision 5e/PF2e: low light в no light), **Limit Field of Vision** (арка), **Bright Light**, **Low Light**, **Directional Light** (угол), **Total Light**, **Light Multiplier** (%), **All Players See Light**.
- Страница: **Explorer Mode** (исследованное — серым; отдельные Edit/Reset explored areas), **Daylight Mode** (яркость), **Update on Token Drop**, **GM Darkness Opacity**, Darkness Effect (Overlay / Dark Fog / Pale Mist), «Only Update on Drop».
- Токены без vision не видят ничего — частая причина «у меня тёмный экран»; официальный чеклист ([Dynamic Lighting Checklist](https://help.roll20.net/hc/articles/360044771413)).

Ключевое архитектурное ограничение: **туман и свет — это два независимых слоя/системы**, комбинировать их надо вручную (двойная работа ГМ).

## A5. Бой: Turn Tracker, HP, условия

Источник: [Turn Tracker](https://help.roll20.net/hc/en-us/articles/360039178634-Turn-Tracker) (17.10.2025).

| Функция | Статус |
|---|---|
| Turn Tracker (инициатива) | ✅ открывает только ГМ, видят все; добавление через ПКМ «Add Turn» |
| Бросок инициативы | ✅ `/roll 1d20+5 &{tracker}`; `&{tracker:+}` / `&{tracker:-}` для ±; drag&drop результата броска в числовое поле |
| Автосортировка | ✅ «Automatically Sort New Entries»; сортировка численно/алфавитно/по масти карт |
| Ходы/раунды | ✅ стрелки вперёд/назад; список прокручивается |
| Кастомные записи | ✅ «Add Custom Item» + **Round Calculation** (`+1`/`-1` каждый круг) и доступ `@{tracker|Item Name}` |
| Условия (conditions) | ⚠️ как отдельных объектов нет; статусы — маркеры-иконки на токене (40+), иконки эффектов из листа 5e |
| Автоматизация боя | ❌ намеренно не автоматизировано (цитата: «This feature is intentionally not automated»); автотрекинг HP — только через лист/мод |
| Turn Tracker на нескольких страницах | ✅, игрок видит только записи своей страницы |
| «Roll20 Combat Master» | ❌ такой встроенной фичи нет; вторичные обзоры называют автоматизацию боя слабым местом Roll20 |

## A6. Листы персонажей и компендиум

| Аспект | Roll20 |
|---|---|
| Система листов | шаблон выбирается при создании игры/в Game Settings; лист = слой представления над системой Attributes |
| Кастомные листы | ✅ только Pro (HTML/CSS редактор в Game Settings) |
| Community-листы | ✅ GitHub `Roll20/roll20-character-sheets`; правка форка — Pro |
| Лист D&D 5e by Roll20 | ✅ основной; Repeating Sections, sheet roll buttons, drag&drop кнопок на Quick Bar |
| **Charactermancer** | ✅ пошаговое создание/левелап для D&D 5e (упоминается в [Character Sheet Tips](https://help.roll20.net/hc/en-us/articles/360037773373-Character-Sheet-Tips-and-Tricks); отдельная статья Help Center — в основном на не-англ. локалях, напр. [ja](https://help.roll20.net/hc/ja/articles/360039644133)) |
| Компендиум | ✅ `roll20.net/compendium`; drag&drop из компендиума в лист/журнал; SRD 5e — бесплатно; купленные книги/аддоны открываются в игре с включённым шерингом (Owned Content Sharing) |
| Шеринг контента | ограничен тарифом (Free 1 игра/5 игроков … Elite 20 игр/15 игроков) |
| Экспорт/перенос персонажей | 3 переноса на игру (Free), без лимита (Plus+) |
| Точность фактов о компендиуме | ⚠️ официальная вики (`wiki.roll20.net/Compendium`) недоступна (403), поэтому детали SRD/купленного контента — «по состоянию на 2026, не подтверждено официальной страницей» |

## A7. Дайсы

Источник: [Dice Reference](https://help.roll20.net/hc/en-us/articles/360037773133-Dice-Reference) (13.08.2026), [3D Dice](https://help.roll20.net/hc/en-us/articles/360039715613-3D-Dice).

- Движок — **QuantumRoll**; отдельный веб-роллер `tools.roll20.net/dice-roller` (часть возможностей там не работает).
- Базовое: `/roll NdX`, произвольное число сторон, Fate/Fudge `NdF`, вычисляемые кубы `(N+Y)dX`.
- Модификаторы: успехи/провалы (`3d6>3f1`), exploding (`!`), compounding (`!!`), penetrating (`!p`), keep/drop (`kh/kl/dh/dl`), reroll (`r`/`ro`), сортировка (`sa/sd`), групповые броски `{...}`.
- Криты/фамблы на отображение: `cs>10`, `cf<3`.
- Inline-броски `[[3d6]]`, вложенные; roll queries `?{...}`; math-only; floor/ceil/round/abs.
- Секретность: `/gmroll` (видит ГМ и бросающий), `/sr` (только ГМ, бросающий видит факт), `/ssr` (не видит никто, кроме ГМ) — сильная фича, редко встречается в других VTT.
- Roll Templates (`&{template:default} {{name=...}}`) — стилизованный вывод в чат, часть системы листов.
- **3D-дайсы**: 🆓 фича пользователя (My Settings → Enable 3D Dice), WebGL; «Dice Agency» — ручной бросок мышью; результат физики определяет итог; цвет — по цвету игрока; видны другим игрокам с включёнными 3D-дайсами. **Не привязаны к тарифу** — это важно: 3D-дайсы есть и на Free.
- Advantage/disadvantage — не отдельная кнопка, а формула `2d20kh1` / `2d20kl1` или кнопка листа.
- Roll Tables: ⚠️ есть в интерфейсе (Rollable Tables), но отдельной статьи Help Center найти не удалось — считать «есть, детали не подтверждены».

## A8. Журнал, хендауты, страницы, права

| Объект | Что есть |
|---|---|
| Journal | древовидный список персонажей/хендаутов, папки, drag&drop в чат/на карту |
| Handout | запись журнала с изображением и rich-text; «Show to Players» |
| Персонаж (Character) | Bio & Info, Attributes & Abilities, Character Sheet |
| Pages | создание/переименование/архив/удаление, папки страниц, «Player Ribbon» (какая страница видна игрокам), Split the Party |
| Права | на токен — Controlled By; на персонажа — «Can Be Edited & Controlled By»; на хендаут — видимость; на страницу — только ГМ |
| «Игрок как ГМ» (Co-GM) | ✅ есть, но через промоушен игрока в ГМ (полные права ГМ); отдельной ограниченной роли «ассистент» нет |
| Приглашения | ссылка-приглашение на игру + LFG/Player Directory |
| Внешний доступ к журналу | Plus+ (External Journal Access) |

## A9. Макросы, API, Mods (Pro/Elite)

Источник: [Introduction to Mod Scripts](https://help.roll20.net/hc/en-us/articles/360037256714-Introduction-to-Mod-Scripts) (26.09.2026).

- Макросы/Abilities — 🆓 для всех: `#macro`, `%{Char|Ability}`, `@{Char|Attr}`, roll queries.
- **Mod (API) Scripts** — 💰 Pro/Elite. JavaScript в песочнице; **sandbox v1.0 и v1.5** (`campaign().sandboxversion`); события, объекты, чат, утилиты; one-click **Script Library**; форум мод-скриптов.
- Моды умеют: двигать объекты, ставить маркеры, бросать кубы, читать/писать атрибуты, управлять светом/дверями (официально: двери/окна «set up to work with Mods»), рисовать, менять turn order.
- Ограничения: только ГМ-сторона (игра создана подписчиком), моды не работают у игроков, лимиты времени выполнения песочницы, отладка в отдельной консоли.
- Коллекция модов сообщества огромна (Power Cards, TokenMod, GroupInitiative, DoorKnocker и т. п.). ⚠️ Точный список/цифры не подтверждаются официальной страницей — только форум и вики.

## A10. Голос/видео, музыка, «видеостена»

| Возможность | Roll20 |
|---|---|
| Голос/видео | ✅ встроено (WebRTC-звонок в кампании); качество среднее, большинство групп уходит в Discord — [(saga20, обзор 2026)](https://saga20.com/blog/roll20-vs-foundry-vtt-2026/) |
| Видеостена / «все игроки лицом» | ⚠️ только сетка видеоколонок встроенного звонка |
| **Jukebox** | ✅ [Jukebox](https://help.roll20.net/hc/en-us/articles/360039178714-Jukebox): My Audio (загрузка mp3/ogg/flac, до 20 МБ), Tabletop Audio, BattleBards, Incompetech; плейлисты (1 уровень вложенности), loop, громкость на трек, Play on Load на странице, «Now Playing» для игроков, вынос в отдельное окно |
| Синхронизация музыки | ✅ играет у всех клиентов; GM-плейлист игрокам не виден |
| Синхронизация видео/«смотреть вместе» | ❌ |
| Карты/ассеты | загрузка своих (лимиты по тарифу), библиотека Marketplace, Token Sets |

## A11. Права, приглашения, мобильное приложение

- Игроки заходят по ссылке, аккаунт Roll20 нужен (бесплатный); анонимного входа нет.
- Приглашение: ссылка +`Join a Game`; заявки обрабатываются вручную.
- Мобильное приложение: **нативное приложение Roll20 выводится из эксплуатации** — официальный анонс в блоге ([We're Retiring the Roll20 Mobile App…](https://blog.roll20.net/posts/were-retiring-the-roll20-mobile-app-to-build-something-better-heres-why/)). Точная дата отключения и судьба замены на странице не читаются автоматически (JS-рендер) — **не подтверждено**. Что точно есть: браузер на планшете/телефоне (адаптив ограничен) и отдельный веб-инструмент **Roll20 Characters** (`app.roll20.net/characters`).
- Вторичный обзор 2026 фиксирует «Mobile access: Yes (browser), Limited» — [(saga20)](https://saga20.com/blog/roll20-vs-foundry-vtt-2026/).

## A12. Главные жалобы пользователей

| Претензия | Подтверждение |
|---|---|
| **Dynamic Lighting тормозит / жрёт память** | официальные ветки форума: [«New Dynamic Lighting lags horribly»](https://app.roll20.net/forum/post/10705870/new-dynamic-lighting-lags-horribly/), [«Legacy lighting completed polygons cause… memory hog»](https://app.roll20.net/forum/post/11407399/legacy-lighting-completed-polygons-cause-a-nasty-bug-dynamic-lighting-memory-hog/), [«Dynamic lighting problem»](https://app.roll20.net/forum/post/11131843/dynamic-lighting-problem); косвенно — сама документация вводит «Only Update on Drop» и раздел System Requirements |
| **Качество/надёжность света и тумана** | Advanced FoW: рваные края у стен не по сетке, нельзя сбросить туман одному токену, кап 50k клеток — [официально](https://help.roll20.net/hc/en-us/articles/360037774493-Advanced-Fog-of-War-Classic-VTT) |
| **UI «из 90-х», перегруженность** | «The interface isn't beautiful, but it's straightforward» — [saga20](https://saga20.com/blog/roll20-vs-foundry-vtt-2026/) |
| **Динамический свет за подписку** | DL только Plus+ ([Feature Breakdown](https://help.roll20.net/hc/en-us/articles/360037774633-Feature-Breakdown)); типичный сценарий «купил модуль — всё равно нужен Plus, чтобы увидеть свет» |
| **Дорого вдолгую** | Pro ≈ $240 за 2 года против $50 единоразово у Foundry — расчёт в [saga20](https://saga20.com/blog/roll20-vs-foundry-vtt-2026/) |
| **Нет оффлайна/селф-хостинга** | архитектура облачная, «Dedicated Online Hosting» — [Feature Breakdown](https://help.roll20.net/hc/en-us/articles/360037774633-Feature-Breakdown) |
| **Бой не автоматизирован** | Turn Tracker «intentionally not automated» — [официально](https://help.roll20.net/hc/en-us/articles/360039178634-Turn-Tracker) |
| **Лимиты хранилища и размера файлов** | 100 МБ / 4 ГБ / 10 ГБ / 50 ГБ — [Feature Breakdown](https://help.roll20.net/hc/en-us/articles/360037774633-Feature-Breakdown) |
| **Потеря данных / архивы** | архивная страница всё равно грузится при старте игры — [Page Settings](https://help.roll20.net/hc/en-us/articles/360039675373-Page-Settings); отдельные жалобы на потерю данных не проверены (вики/форумы недоступны для выборки) |

---

# B. Owlbear Rodeo 2

## B1. Модель, комнаты, тарифы

- Полностью браузерный клиент, облачный хостинг Owlbear, **без установки**; адаптив под мобильные ([Getting Started](https://docs.owlbear.rodeo/docs/getting-started/)).
- **Room-based модель**: ГМ создаёт комнату в профиле и раздаёт ссылку; игроки попадают в waiting room и запрашиваются на вход ([Rooms](https://docs.owlbear.rodeo/docs/rooms/)).
- **Данные не привязаны к комнате**: удаление комнаты не удаляет сцены/картинки; имя комнаты = часть URL и после создания не меняется.
- **Анонимные игроки** ✅: аккаунт нужен только чтобы иметь свои сцены/картинки.
- Тарифы ([Managing your Subscription](https://docs.owlbear.rodeo/docs/managing-your-subscription/)):

| План | Цена | Комнаты | Хранилище | Кастомизация |
|---|---|---|---|---|
| **Nestling** (free) | $0 🆓 | 2 | ~200 МБ (по migration guide) | случайные имена/фоны |
| **Fledgling** | 💰 «цена в checkout», публично не публикуется | 10 | 5 ГБ | свои имена/фоны |
| **Bestling** | 💰 «цена в checkout» | 25 | 10 ГБ | свои имена/фоны + ранний доступ к новым фичам |

- Публичных цен на официальных страницах нет: и доки, и вторичные трекеры ([rottenwifi](https://rottenwifi.com/services/owlbear-rodeo/pricing/), [macmyths](https://macmyths.com/apps/owlbear-rodeo/)) фиксируют «on request / показано в checkout». Любые «$X/мес» из обзоров — **не подтверждено**.
- Лимиты изображений (релиз 2.3, по [macmyths](https://macmyths.com/apps/owlbear-rodeo/)): 25 МБ / 67 Мп для Nestling и Fledgling; 50 МБ / 144 Мп для Bestling.
- Релиз 2.4: **Forecast** — computer-vision детекция комнат на карте для ускорения тумана, бета сначала для Bestling (там же).
- Экспорт/импорт данных между VTT: есть только миграция 1.0 → 2.0 ([Migration](https://docs.owlbear.rodeo/docs/migration/)), общего импорта из Roll20/Foundry нет.

## B2. Игровое поле: сцены, сетки, слои, измерение, AoE, туман

**Сцена** — бесконечный холст; карта — отдельная сущность в библиотеке, переиспользуется между сценами ([Scenes](https://docs.owlbear.rodeo/docs/scenes/)).

| Параметр сетки | Значения |
|---|---|
| Тип | square, hex horizontal, hex vertical, **isometric** (+ Axonometry: Isometric/Dimetric) |
| Оформление | цвет (dark/light/highlight), прозрачность, толщина линии, тип линии (solid/dashed/dotted) |
| Размер клетки | задаётся в Scene Controls (масштаб измерения) |
| Снап | вкл/выкл глобально |
| Способ измерения | настраивается на сцену (см. ниже) |

Слои (по [Images](https://docs.owlbear.rodeo/docs/images/), снизу вверх): **Map → (grid) → Prop → Mount → Character → Attachment → Note**; отдельно **Drawing layer** (над картой и сеткой, ниже картинок). Иерархия задана типом картинки, а не ручным z-order.

**Измерение** ([Measure](https://docs.owlbear.rodeo/docs/measure/)): режимы ruler и movement; перманентная линейка (двойной клик); типы — Chessboard (5e), Alternating Diagonal (3.5e/PF), Euclidean, Manhattan; для гексов — Hexagon/Euclidean; масштаб на сцену (5 ft, 1.5 m, 5 mi …), от масштаба зависит точность вывода.

**AoE-шаблоны**: встроенных шаблонов «конус/линия/сфера по правилам 5e» нет — есть фигуры рисования (Circle, Triangle, Line, Rectangle, Hexagon, Polygon, Brush, Marker) с заливкой/обводкой, Join/Trim и слоем ([Drawing](https://docs.owlbear.rodeo/docs/drawing/)). Правильные шаблоны — расширения **AoE Shapes**, **Ranges**, **Hex Range**.

**Туман войны**: только **simple fog**, вручную ([Fog](https://docs.owlbear.rodeo/docs/fog/)):
- фигуры: Rectangle, Polygon, Circle, Triangle, Hexagon, Brush;
- **single-layer fog** — новая фигура автоматически вычитается из соседних (не перекрываются); режим Multilayer отключает это;
- действия: Cut (вырезать область), Join (объединить фигуры), Trim, Fog Fill (залить всё тьмой);
- туман — геометрия на сцене, а не «память по токенам»; per-player тумана нет.

**Динамическое освещение**: ❌ в ядре отсутствует. Реализуется расширениями: **Dynamic Fog** (официальное, «walls, doors and lights for a simple dynamic fog experience»), **Smoke & Spectre!** (per-player fog), **Peekaboo** (проверка видимости через стены из Dynamic Fog) — [extensions.owlbear.rodeo](https://extensions.owlbear.rodeo/).
**Погода**: ❌ в ядре; официальное расширение **Weather** (rain/snow) — там же.
**Ночное зрение/vision**: ❌ нет как системы; только через расширения.

## B3. Токены и ассеты

- Типы картинок = типы токенов: Map, Prop, Mount, Character, Attachment, Note ([Images](https://docs.owlbear.rodeo/docs/images/)).
- Импорт: Visible, Locked, **Size** (Auto по типу/разрешению, Set DPI, Set Columns, Set Rows), Rotation, Text Color/Font/Default Text.
- **Attachment** при перетаскивании на персонажа автоматически прикрепляется (статусы, оружие); **Mount** — при посадке персонажа на маунта тот прикрепляется.
- `Duplicate` автоматически инкрементит номер («Goblin 1» → «Goblin 2») — приятная мелочь для энкаунтеров.
- Перемещение/поворот/масштаб — ручки; Lock; Hide/Show (для ГМ полупрозрачно, игрокам невидимо); Layer (сменить тип-слой); Replace Image; Align Image; Accessibility (имя/описание для скринридеров).
- **Свои данные токена (HP, AC, статусы) в ядре отсутствуют** — Stat Bubbles for D&D, Token Counter, Game Master's Grimoire, Owl Trackers, Colored Rings (официальное), Condition Markers и др.
- Библиотека: Starter Sets (карты, токены, туториалы) + загрузка своих; папки/коллекции в Asset Manager; отдельного «магазина токенов» нет — есть сторонние паки-расширения (Tabletop Tokens: 4000+ иконок).
- «Лист персонажа как расширение»: **Chronicle!** (party-shared sheets), DummySheet, Shadow Sheet, Sheet from Beyond (внешние URL листов), Owl20 (мост к D&D Beyond), External Links.

## B4. Расширения и SDK

- **Extension SDK** — публичный, TypeScript, `@owlbear-rodeo/sdk` ([Getting Started](https://docs.owlbear.rodeo/extensions/getting-started/), [GitHub](https://github.com/owlbear-rodeo/sdk)).
- Точка входа — `manifest.json` (имя, автор, версия, background page/action); установка — по ссылке на манифест; расширения включаются на комнату.
- Точки интеграции UI: **Action** (кнопка слева вверху → iframe-поповер), **Action Popover**, **Context Menu** (ПКМ по выделенному), **Popover**, **Tool** + **Tool Mode** (свои клики/драги на канвасе), **Tool Action** (кнопка в панели инструментов).
- Галерея: [extensions.owlbear.rodeo](https://extensions.owlbear.rodeo/) — фильтры по тегам (Dice, Fog, Tool, Content Pack, Drawing, Audio, Combat, Automation, Other), бейдж **Verified**, тег **Built By Owlbear**. На момент сбора — ~100+ расширений в каталоге.
- Официальные (Built By Owlbear): Initiative Tracker, **Dice**, Colored Rings, Outliner, Prefabs, **Dynamic Fog**, **Weather**, Ranges, Trackers.
- Песочница: расширение — это веб-страница в iframe с доступом к SDK (сцена, items, tool, players, notification, context menu); отдельной «permission-модели» для расширений в доках не описано — фактически расширение видит сцену и может её менять.
- Монетизация расширений: официального встроенного стори/подписки нет; авторы продают/распространяют сами (battle-system.com, dddice и др.). **Не подтверждено** наличие revenue-share.

## B5. Дайсы

- Встроенного роллера в ядре нет; официальное расширение **Dice** ([карточка](https://extensions.owlbear.rodeo/dice)): 3D-кубы с физикой, трей, **синхронизация броска по сети в реальном времени** (все видят), приватные броски (иконка глаза), **бонус и advantage/disadvantage** (кнопка +/-), история последних бросков, превью трея других игроков.
- Формульного языка (как `/roll 2d20kh1`) в ядре и в базовом Dice нет; расширенное — **Dice+** (кастомные кубы, формулы), **dddice** (тысячи скинов + интеграция с D&D Beyond/Stream Deck), **Bones!** (скрытые броски + лог), **Tables+** (взвешенные roll-таблицы).
- История бросков в чате: ❌ (чата нет) — только «recent rolls» в самом расширении.

## B6. Что ещё есть/нет в Owlbear

| Функция | Статус |
|---|---|
| Чат | ❌ в ядре; расширения **Rumble!** (DM/сейфти-карты/кубы), **Journal!**, **Bones!** |
| Заметки ГМ | ⚠️ тип картинки **Note** — rich-text поверх картинки, видимость как у картинки; отдельного «журнала» нет (Journal! / GM Vault / DM Screen — расширения) |
| Инициатива | 🧩 официальное **Initiative Tracker** (общий вид для ГМ и игроков); альтернативы — Pretty Sordid, SIT, Combat Tracker, Battle Board |
| Компендиум | ❌ нет; ближайшее — Owl20 (D&D Beyond), PDF!, GM Vault |
| Боевая автоматика | ❌ нет; максимум — расширения (Clash!, Phases Automated, Causality) |
| Музыка/аудио | ❌ в ядре; расширения **Tracks** (синхронизированное проигрывание), Dragon Disco |
| Видеостена / показ игрокам | ✅ **Casting** через Presentation API (Chromecast/внешний дисплей; Android не поддерживается), AirPlay через macOS, управление **Sync View** ([Casting](https://docs.owlbear.rodeo/docs/casting/)) |
| Права игроков | ✅ по каждому слою: create/update/delete; **Owner Only** для персонажей + назначение владельца ГМом ([Permissions](https://docs.owlbear.rodeo/docs/permissions/)) |
| Co-GM | ❌ отдельной роли нет (в доках не описано) |
| Приглашение | ✅ ссылка на комнату + approve заявки |
| Анонимные игроки | ✅ |
| Мобильное | ✅ адаптивный веб (отдельного нативного приложения в доках не описано) |
| Оффлайн/селф-хостинг | ❌ только облако Owlbear |
| Туман «по памяти токена» | ❌ |
| Сетка/измерение per scene | ✅ |
| Показ картинки игрокам (handout) | ⚠️ через Casting/скрытие-показ объектов, отдельного «показать хендаут» нет |

## B7. Сильные стороны

1. **Время до первой игры** — минуты: комната → сцена из starter set → токены → ссылка. Ноль установки.
2. **Игроки без аккаунтов** — снижает порог входа для случайных игроков.
3. **Скорость и «просто работает»**: 3D-дайсы синхронизируются, движение/линейка/туман — мгновенные, нет тяжёлого света.
4. **Чистый минимализм UI**: панель инструментов, dock по типам картинок, контекстные меню — почти нет «страшных» диалогов.
5. **Туман-геометрия** с single-layer/Cut/Join — быстрое «закрашивание» данжа без клика по клеткам (и без тормозов, в отличие от клеточного A-FoW).
6. **SDK, ставший де-факто стандартом** — вокруг десятки независимых расширений под любые системы; ядро остаётся маленьким.
7. **Casting/Sync View** — реально полезно для игр «вживую» с ТВ/проектором.
8. **Дёшево на входе** (free 2 комнаты достаточно для одной кампании).

## B8. Известные жалобы и ограничения

| Ограничение | Следствие |
|---|---|
| Нет динамического света/vision в ядре | каждый ГМ ставит Dynamic Fog/Smoke & Spectre и рисует стены сам; поведение и качество зависят от расширения |
| Нет чата, музыки, листов, компендиума | «зоопарк» из 4–8 расширений на комнату; каждое — сторонний код в вашей сессии |
| Нет боевой автоматики | HP/условия/инициатива — вручную или расширениями |
| Туман только ручной | нет «памяти исследования» на игрока/токен; большие карты — много рисования |
| Цены не публикуются | невозможно спланировать бюджет без похода в checkout (фиксируют и [rottenwifi](https://rottenwifi.com/services/owlbear-rodeo/pricing/), и [macmyths](https://macmyths.com/apps/owlbear-rodeo/)) |
| Лимиты free-плана | 2 комнаты, ~200 МБ, случайные имена/фоны, 25 МБ / 67 Мп на файл |
| Только облако | оффлайн/селф-хостинг невозможен, данные у вендора |
| Нет ролей кроме ГМ/игрок | нет Co-GM/ассистента, нет тонких ролей |
| Иерархия только по типам | нельзя свободно менять z-order внутри слоя (только через Layer/расширения Flip!) |
| Документация не описывает API-лимиты расширений | аудит безопасности расширений — на совести ГМа |

---

# C. Итоговые таблицы

## C1. «Поле/панель → Roll20 → Owlbear»

| # | Элемент | Roll20 | Owlbear Rodeo 2 |
|---|---|---|---|
| 1 | Карта/фон | ✅ Map & Background | ✅ Maps (отдельно от сцены, переиспользуются) |
| 2 | Порядок слоёв | ✅ Map → Tokens → GM Info → Foreground → DL | ✅ Map → grid → Prop → Mount → Character → Attachment → Note (+Drawing) |
| 3 | Слой только для ГМ | ✅ GM Info Overlay | ⚠️ через Hide (объект полупрозрачен ГМу) |
| 4 | Foreground/объекты над токенами | ✅ (Plus/Pro) | ⚠️ Prop ниже персонажа; верхний слой — Note |
| 5 | Квадратная сетка | ✅ | ✅ |
| 6 | Гекс-сетка | ✅ H и V | ✅ H и V |
| 7 | Изометрическая сетка | ❌ | ✅ (+Dimetric) |
| 8 | Масштаб/единицы на страницу | ✅ ft/m/km/mi + Daggerheart Range | ✅ на сцену (ft/m/km/mi, точность) |
| 9 | Типы измерения дистанции | ✅ 5e/4e, 3.5/PF, Euclidean, Manhattan, Hex | ✅ Chessboard, Alternating Diagonal, Euclidean, Manhattan, Hexagon |
| 10 | Линейка | ✅ | ✅ (временная + перманентная) |
| 11 | Линейка с режимом движения | ⚠️ через измерение при drag | ✅ явный movement mode |
| 12 | Измерение пути с изломами | ⚠️ ограниченно | 🧩 Bendy Ruler / Segmentable Ruler / Path Measure |
| 13 | AoE-шаблоны 5e (конус/линия/сфера) | ⚠️ фигуры рисования + моды | 🧩 AoE Shapes / Ranges / Hex Range |
| 14 | Размер токена (клетки/DPI) | ✅ Set Dimensions + снап | ✅ Auto / DPI / Columns / Rows |
| 15 | Имя/подпись на токене | ✅ nameplate | ✅ label + rich text |
| 16 | HP-бары на токене | ✅ 3–4 бара, привязка к атрибутам | 🧩 Stat Bubbles / GM's Grimoire / Token Counter |
| 17 | Статус-маркеры/условия | ✅ 40+ маркеров, цифры 0–9 | 🧩 Colored Rings (офиц.), Condition Markers, Marked! |
| 18 | Ауры | ✅ 2 ауры, 6 стилей | 🧩 Token Auras / Auras and Emanations |
| 19 | Vision/свет токена | ✅ (Plus/Pro) | 🧩 Dynamic Fog |
| 20 | Вращение/масштаб | ✅ | ✅ |
| 21 | Привязка токена к листу | ✅ Represents Character | ❌ (нет листа; Attachment/Mount — про объекты) |
| 22 | Вложения (оружие/эффекты) | ⚠️ маркеры/токены на слое | ✅ Attachment (авто-прикрепление) |
| 23 | Маунты | ❌ | ✅ Mount |
| 24 | Простой туман (ручной) | ✅ Fog of War / Hide-Reveal Mask (все тарифы) | ✅ Fog (фигуры, single-layer, Cut/Join, Fog Fill) |
| 25 | Туман с «памятью» исследования | ✅ Advanced FoW (Plus/Pro) | ❌ |
| 26 | Динамическое освещение | ✅ (Plus/Pro) | ❌ (расширение) |
| 27 | Стены | ✅ (Plus/Pro) | 🧩 Dynamic Fog |
| 28 | Двери/окна (locked/secret) | ✅ (Plus/Pro) | 🧩 Dynamic Fog (двери) |
| 29 | Источники света на карте | ✅ токены-светильники | 🧩 Dynamic Fog |
| 30 | Ночное зрение / darkvision | ✅ Night Vision, Nocturnal Vision | ❌ |
| 31 | Погода | ❌ (только эффекты тьмы) | 🧩 Weather (офиц.) |
| 32 | Инициатива | ✅ Turn Tracker + `&{tracker}` | 🧩 Initiative Tracker (офиц.) |
| 33 | Раунды/счётчики ходов | ✅ + Round Calculation, `@{tracker|...}` | 🧩 Trackers / SIT / Count! |
| 34 | Условия как объекты | ❌ (маркеры) | ❌ |
| 35 | Лист персонажа | ✅ (+кастом Pro) | 🧩 Chronicle!, DummySheet, Shadow Sheet, Sheet from Beyond |
| 36 | Компендиум | ✅ (SRD + купленное) | ❌ (Owl20→D&D Beyond, PDF!, GM Vault) |
| 37 | Генерация персонажа (Charactermancer) | ✅ D&D 5e | ❌ (через D&D Beyond/Owl20) |
| 38 | Броски из листа | ✅ sheet roll buttons | 🧩 внутри расширений-листов |
| 39 | 3D-дайсы | ✅ QuantumRoll, физика, Dice Agency (Free) | ✅ Dice (офиц.), физика + синхрон |
| 40 | Формульный язык бросков | ✅ очень богатый (`kh/ro/!/!!/>N` и т.д.) | ❌ (Dice+ частично) |
| 41 | Advantage/disadvantage | ⚠️ формулой/кнопкой листа | ✅ кнопка в Dice |
| 42 | Скрытые/ГМ-броски | ✅ /gmroll, /sr, /ssr | ✅ приватный бросок в Dice |
| 43 | Roll-таблицы | ⚠️ есть, детали не подтверждены | 🧩 Tables+ / Loot Tables |
| 44 | Журнал/хендауты | ✅ Journal + Handout | ❌ (Note / Journal! / GM Vault) |
| 45 | Заметки на карте | ✅ GM Info Overlay + текст | ✅ Text / Note |
| 46 | Страницы/папки | ✅ Pages, папки, архив | ✅ Scenes, коллекции/папки, несколько карт в сцене |
| 47 | Права игроков по слоям | ⚠️ только через владение объектом | ✅ create/update/delete по каждому слою |
| 48 | Owner Only для персонажей | ⚠️ Controlled By | ✅ Owner Only + назначение владельца |
| 49 | Co-GM/ассистент | ⚠️ промоушен до ГМ | ❌ |
| 50 | Вход по ссылке | ✅ (+аккаунт) | ✅ (+анонимно) |
| 51 | Чат | ✅ текстовый + ролл-лог | ❌ (Rumble! / Bones!) |
| 52 | Голос/видео | ✅ встроено | ❌ |
| 53 | Музыка/аудио | ✅ Jukebox (4 источника + свои файлы) | 🧩 Tracks / Dragon Disco |
| 54 | Макросы | ✅ (Free) | ❌ |
| 55 | API/скрипты | ✅ Mod API (Pro/Elite, JS) | ✅ Extension SDK (JS/TS, любому) |
| 56 | Каталог расширений | ⚠️ форум/вики + Script Library | ✅ официальная галерея + Verified |
| 57 | Показ игрокам на ТВ/проектор | ⚠️ Player View/Rejoin as Player | ✅ Casting + Sync View |
| 58 | Мобильное приложение | ❌ нативное выводится из эксплуатации; браузер | ✅ адаптивный веб |
| 59 | Оффлайн/селф-хостинг | ❌ | ❌ |
| 60 | Лимиты хранилища | 100 МБ / 4 / 10 / 50 ГБ | ~200 МБ / 5 / 10 ГБ |

## C2. Сводка: что обязательно тащить, что переосмыслить

### Тащить как есть (проверенные ожидания игроков)
1. **Turn Tracker с ручным контролем + `&{tracker}`-эквивалент**: авто-сортировка, drag&drop значения, счётчики-раунды через «кастомные записи с ±N за круг». Roll20-модель проще и гибче «полной автоматизации».
2. **Три уровня тумана**: ручной (все), «память исследования» (авто), динамический свет. Именно наличие **всех трёх** — то, чего нет ни у Owlbear, ни (по качеству) у Roll20.
3. **Секретные броски трёх уровней** (`/gmroll`, `/sr`, `/ssr`) — дёшево в реализации, высоко ценится ГМами.
4. **Бары/маркеры/ауры на токене** с привязкой к атрибутам — стандарт индустрии; 3–4 бара, 40+ маркеров, 2 ауры.
5. **3D-дайсы бесплатно + «Dice Agency»** — в Roll20 это бесплатная фича, а не премиум.
6. **Анонимный вход игроков по ссылке** (Owlbear) — сильно снижает трение; в Roll20 этого нет, и это заметная жалоба новичков.
7. **Casting/Sync View** (Owlbear) — обязательна для игр «вживую» с ТВ.
8. **Слои по типам контента + Attachment/Mount** (Owlbear) — иерархия «по типу» проще для новичка, чем ручной z-order.
9. **Туман-геометрия с single-layer/Cut/Join** (Owlbear) — самый быстрый способ закрасить данж; клеточная модель Roll20 объективно хуже.
10. **Jukebox-модель**: свои файлы + встроенные бесплатные библиотеки + плейлисты + «play on load» на сцене.
11. **Каталог расширений с бейджем Verified** и SDK, позволяющим рисовать свои tool-режимы — это то, что делает Owlbear расширяемым без раздувания ядра.

### Переосмыслить (не копировать)
12. **Платный динамический свет.** В Roll20 DL — главный пейволл, а качество при этом — главная жалоба. В нашем десктопе свет должен быть в базе и быстрым (нативно, без браузера).
13. **Клеточный туман.** Roll20 Advanced FoW: рваные края, кап 50k клеток, нет сброса одному токену. Нужен туман как **геометрия + маска на текстуре** (GPU), с per-player «памятью», а не сеткой.
14. **Двойная система «туман + свет».** У Roll20 это два независимых слоя и двойная работа ГМа. Нужна одна модель «видимость» с двумя режимами отрисовки.
15. **Бой «намеренно не автоматизирован»** и «лист = HTML/CSS-шаблон». Для 5e/5.5e нужен типизированный лист + правила как данные (conditions, концентрация, слоты), иначе повторим главную боль Roll20.
16. **Отсутствие оффлайна и зависимость от серверов.** Оба конкурента — только облако. Хостинг у ГМ (наш бриф) — уникальное преимущество, но нужно решить: обнаружение в LAN, NAT-traversal, релей-fallback, версионирование состояния.
17. **«Расширения = произвольный сторонний JS в вашей комнате».** У Owlbear песочница фактически без модели прав. У нас: WASM/изолированные плагины, декларативные манифесты прав, подпись.
18. **Раздутый UI и скрытые состояния.** У Roll20 половина проблем ГМов — «почему темно/почему не видно» из-за комбинации Vision/Light/Explorer/Daylight/Darkness Effect. Нужны пресеты («факел», «тёмный данж») и валидатор настройки.
19. **Отсутствие единого «показать игрокам».** Ни Roll20, ни Owlbear не умеют нормально показать handout/карту всем как «слайд». У нас — отдельный канал «сцена/слайд/хендаут» с очередью.
20. **Лимиты хранилища и мутные цены.** У Owlbear цены вообще не публикуются, у Roll20 хранилище — товар. В нативном приложении ассеты должны лежать у ГМ; монетизация — только сам софт, без «доплати за свет».
21. **Мобильность.** Нативное приложение Roll20 закрывают; Owlbear решает адаптивом. Разумный путь — тонкий веб/планшетный клиент-«вьюер» к хосту ГМа, без полного функционала.

### Открытые вопросы (для следующих задач)
- Компендиум 5e/5.5e: SRD-only или лицензионные данные (CC-BY-4.0 SRD 5.2 / 5.1) — юридический вопрос, не технический.
- Импорт существующих кампаний Roll20 (экспорт `campaign.json`?) — официального экспорта «всё» нет; нужно проверять отдельно.
- Синхронизация 3D-дайсов физикой между Rust-клиентами: детерминированная физика или «хост считает, клиенты проигрывают» — влияет на архитектуру сети.

---

## Источники

**Roll20 (официальные)**
- [Feature Breakdown (тарифы)](https://help.roll20.net/hc/en-us/articles/360037774633-Feature-Breakdown)
- [Layers](https://help.roll20.net/hc/en-us/articles/360039675053-Layers) · [Page Settings](https://help.roll20.net/hc/en-us/articles/360039675373-Page-Settings)
- [What Is Dynamic Lighting?](https://help.roll20.net/hc/en-us/articles/4403801475479-What-Is-Dynamic-Lighting) · [Setting Up Dynamic Lighting](https://roll20partners.zendesk.com/hc/en-us/articles/10269638920599-Setting-Up-Dynamic-Lighting) · [Page Settings for DL](https://help.roll20.net/hc/en-us/articles/360052521913-Page-Settings-For-Dynamic-Lighting) · [Token Settings for DL](https://help.roll20.net/hc/en-us/articles/360051754954-Token-Settings-for-Dynamic-Lighting)
- [Token Features](https://help.roll20.net/hc/en-us/articles/360039674573-Token-Features)
- [Turn Tracker](https://help.roll20.net/hc/en-us/articles/360039178634-Turn-Tracker)
- [Dice Reference](https://help.roll20.net/hc/en-us/articles/360037773133-Dice-Reference) · [3D Dice](https://help.roll20.net/hc/en-us/articles/360039715613-3D-Dice)
- [Fog of War](https://help.roll20.net/hc/en-us/articles/360037774513-Fog-of-War-Classic-VTT) · [Advanced Fog of War](https://help.roll20.net/hc/en-us/articles/360037774493-Advanced-Fog-of-War-Classic-VTT)
- [Jukebox](https://help.roll20.net/hc/en-us/articles/360039178714-Jukebox)
- [Character Sheet Tips and Tricks](https://help.roll20.net/hc/en-us/articles/360037773373-Character-Sheet-Tips-and-Tricks)
- [Introduction to Mod Scripts](https://help.roll20.net/hc/en-us/articles/360037256714-Introduction-to-Mod-Scripts)
- [Marketplace](https://marketplace.roll20.net) · [Roll20 Characters](https://app.roll20.net/characters)
- [Блог: вывод мобильного приложения](https://blog.roll20.net/posts/were-retiring-the-roll20-mobile-app-to-build-something-better-heres-why/)

**Roll20 (жалобы на форуме)**
- [Dynamic Lighting lags horribly](https://app.roll20.net/forum/post/10705870/new-dynamic-lighting-lags-horribly/)
- [Legacy lighting polygons / memory hog](https://app.roll20.net/forum/post/11407399/legacy-lighting-completed-polygons-cause-a-nasty-bug-dynamic-lighting-memory-hog/)
- [Dynamic lighting problem](https://app.roll20.net/forum/post/11131843/dynamic-lighting-problem)

**Owlbear Rodeo (официальные)**
- [Docs: Getting Started](https://docs.owlbear.rodeo/docs/getting-started/) · [Rooms](https://docs.owlbear.rodeo/docs/rooms/) · [Scenes](https://docs.owlbear.rodeo/docs/scenes/) · [Images](https://docs.owlbear.rodeo/docs/images/) · [Fog](https://docs.owlbear.rodeo/docs/fog/) · [Drawing](https://docs.owlbear.rodeo/docs/drawing/) · [Measure](https://docs.owlbear.rodeo/docs/measure/) · [Permissions](https://docs.owlbear.rodeo/docs/permissions/) · [Casting](https://docs.owlbear.rodeo/docs/casting/) · [Managing your Subscription](https://docs.owlbear.rodeo/docs/managing-your-subscription/) · [Migration](https://docs.owlbear.rodeo/docs/migration/)
- [Extension SDK: Getting Started](https://docs.owlbear.rodeo/extensions/getting-started/) · [SDK на GitHub](https://github.com/owlbear-rodeo/sdk)
- [Галерея расширений](https://extensions.owlbear.rodeo/) · [Dice](https://extensions.owlbear.rodeo/dice) · [Dynamic Fog](https://extensions.owlbear.rodeo/dynamic-fog)

**Вторичные (обзоры/трекеры 2026)**
- [saga20: Roll20 vs Foundry 2026](https://saga20.com/blog/roll20-vs-foundry-vtt-2026/) — тарифы, голос/видео, «UI не красив».
- [macmyths: Owlbear Rodeo](https://macmyths.com/apps/owlbear-rodeo/) — лимиты изображений 2.3, хранилище 2.0, Forecast 2.4, «Public API: No».
- [rottenwifi: Owlbear Rodeo pricing](https://rottenwifi.com/services/owlbear-rodeo/pricing/) — планы и отсутствие публичных цен.
