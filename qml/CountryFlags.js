.pragma library

// Territory names from Unicode CLDR 48.0.0 (English and Russian).
// https://github.com/unicode-org/cldr-json/tree/48.0.0
// Unicode License v3 is included in licenses/unicode-cldr.txt.
var territories = {
    AD: ["Andorra", "Андорра"],
    AE: ["United Arab Emirates", "ОАЭ", "UAE", "U A E"],
    AF: ["Afghanistan", "Афганистан"],
    AG: ["Antigua & Barbuda", "Антигуа и Барбуда"],
    AI: ["Anguilla", "Ангилья"],
    AL: ["Albania", "Албания"],
    AM: ["Armenia", "Армения"],
    AO: ["Angola", "Ангола"],
    AQ: ["Antarctica", "Антарктида"],
    AR: ["Argentina", "Аргентина"],
    AS: ["American Samoa", "Американское Самоа"],
    AT: ["Austria", "Австрия"],
    AU: ["Australia", "Австралия"],
    AW: ["Aruba", "Аруба"],
    AX: ["Åland Islands", "Аландские о-ва"],
    AZ: ["Azerbaijan", "Азербайджан"],
    BA: ["Bosnia & Herzegovina", "Bosnia", "Босния и Герцеговина"],
    BB: ["Barbados", "Барбадос"],
    BD: ["Bangladesh", "Бангладеш"],
    BE: ["Belgium", "Бельгия"],
    BF: ["Burkina Faso", "Буркина-Фасо"],
    BG: ["Bulgaria", "Болгария"],
    BH: ["Bahrain", "Бахрейн"],
    BI: ["Burundi", "Бурунди"],
    BJ: ["Benin", "Бенин"],
    BL: ["St. Barthélemy", "Сен-Бартелеми"],
    BM: ["Bermuda", "Бермудские о-ва"],
    BN: ["Brunei", "Бруней"],
    BO: ["Bolivia", "Боливия"],
    BQ: ["Caribbean Netherlands", "Бонэйр, Синт-Эстатиус и Саба"],
    BR: ["Brazil", "Бразилия"],
    BS: ["Bahamas", "Багамы"],
    BT: ["Bhutan", "Бутан"],
    BV: ["Bouvet Island", "о-в Буве"],
    BW: ["Botswana", "Ботсвана"],
    BY: ["Belarus", "Беларусь"],
    BZ: ["Belize", "Белиз"],
    CA: ["Canada", "Канада"],
    CC: ["Cocos (Keeling) Islands", "Cocos Islands", "Кокосовые о-ва"],
    CD: ["Congo - Kinshasa", "Congo (DRC)", "Конго - Киншаса", "Конго (ДРК)"],
    CF: ["Central African Republic", "Центрально-Африканская Республика"],
    CG: ["Congo - Brazzaville", "Congo (Republic)", "Конго - Браззавиль", "Республика Конго"],
    CH: ["Switzerland", "Швейцария"],
    CI: ["Côte d’Ivoire", "Ivory Coast", "Кот-д’Ивуар"],
    CK: ["Cook Islands", "о-ва Кука"],
    CL: ["Chile", "Чили"],
    CM: ["Cameroon", "Камерун"],
    CN: ["China", "Китай"],
    CO: ["Colombia", "Колумбия"],
    CP: ["Clipperton Island", "о-в Клиппертон"],
    CR: ["Costa Rica", "Коста-Рика"],
    CU: ["Cuba", "Куба"],
    CV: ["Cape Verde", "Cabo Verde", "Кабо-Верде"],
    CW: ["Curaçao", "Кюрасао"],
    CX: ["Christmas Island", "о-в Рождества"],
    CY: ["Cyprus", "Кипр"],
    CZ: ["Czechia", "Czech Republic", "Чехия", "Чешская Республика"],
    DE: ["Germany", "Германия", "Германии"],
    DG: ["Diego Garcia", "Диего-Гарсия"],
    DJ: ["Djibouti", "Джибути"],
    DK: ["Denmark", "Дания"],
    DM: ["Dominica", "Доминика"],
    DO: ["Dominican Republic", "Доминиканская Республика"],
    DZ: ["Algeria", "Алжир"],
    EC: ["Ecuador", "Эквадор"],
    EE: ["Estonia", "Эстония"],
    EG: ["Egypt", "Египет"],
    EH: ["Western Sahara", "Западная Сахара"],
    ER: ["Eritrea", "Эритрея"],
    ES: ["Spain", "Испания"],
    ET: ["Ethiopia", "Эфиопия"],
    EU: ["European Union", "Европейский союз"],
    FI: ["Finland", "Финляндия", "Финляндии"],
    FJ: ["Fiji", "Фиджи"],
    FK: ["Falkland Islands", "Falkland Islands (Islas Malvinas)", "Фолклендские о-ва", "Фолклендские (Мальвинские) о-ва"],
    FM: ["Micronesia", "Федеративные Штаты Микронезии"],
    FO: ["Faroe Islands", "Фарерские о-ва"],
    FR: ["France", "Франция", "Франции"],
    GA: ["Gabon", "Габон"],
    GB: ["United Kingdom", "Великобритания", "Британия", "Britain", "Great Britain", "U K", "Великобритании", "Лондон"],
    GD: ["Grenada", "Гренада"],
    GE: ["Georgia", "Грузия"],
    GF: ["French Guiana", "Французская Гвиана"],
    GG: ["Guernsey", "Гернси"],
    GH: ["Ghana", "Гана"],
    GI: ["Gibraltar", "Гибралтар"],
    GL: ["Greenland", "Гренландия"],
    GM: ["Gambia", "Гамбия"],
    GN: ["Guinea", "Гвинея"],
    GP: ["Guadeloupe", "Гваделупа"],
    GQ: ["Equatorial Guinea", "Экваториальная Гвинея"],
    GR: ["Greece", "Греция"],
    GS: ["South Georgia & South Sandwich Islands", "Южная Георгия и Южные Сандвичевы о-ва"],
    GT: ["Guatemala", "Гватемала"],
    GU: ["Guam", "Гуам"],
    GW: ["Guinea-Bissau", "Гвинея-Бисау"],
    GY: ["Guyana", "Гайана"],
    HK: ["Hong Kong SAR China", "Hong Kong", "Гонконг (САР)", "Гонконг"],
    HM: ["Heard & McDonald Islands", "о-ва Херд и Макдональд"],
    HN: ["Honduras", "Гондурас"],
    HR: ["Croatia", "Хорватия"],
    HT: ["Haiti", "Гаити"],
    HU: ["Hungary", "Венгрия"],
    IC: ["Canary Islands", "Канарские о-ва"],
    ID: ["Indonesia", "Индонезия"],
    IE: ["Ireland", "Ирландия"],
    IL: ["Israel", "Израиль"],
    IM: ["Isle of Man", "о-в Мэн"],
    IN: ["India", "Индия"],
    IO: ["British Indian Ocean Territory", "Британская территория в Индийском океане"],
    IQ: ["Iraq", "Ирак"],
    IR: ["Iran", "Иран"],
    IS: ["Iceland", "Исландия"],
    IT: ["Italy", "Италия"],
    JE: ["Jersey", "Джерси"],
    JM: ["Jamaica", "Ямайка"],
    JO: ["Jordan", "Иордания"],
    JP: ["Japan", "Япония", "Японии"],
    KE: ["Kenya", "Кения"],
    KG: ["Kyrgyzstan", "Киргизия"],
    KH: ["Cambodia", "Камбоджа"],
    KI: ["Kiribati", "Кирибати"],
    KM: ["Comoros", "Коморы"],
    KN: ["St. Kitts & Nevis", "Сент-Китс и Невис"],
    KP: ["North Korea", "КНДР"],
    KR: ["South Korea", "Республика Корея"],
    KW: ["Kuwait", "Кувейт"],
    KY: ["Cayman Islands", "о-ва Кайман"],
    KZ: ["Kazakhstan", "Казахстан"],
    LA: ["Laos", "Лаос"],
    LB: ["Lebanon", "Ливан"],
    LC: ["St. Lucia", "Сент-Люсия"],
    LI: ["Liechtenstein", "Лихтенштейн"],
    LK: ["Sri Lanka", "Шри-Ланка"],
    LR: ["Liberia", "Либерия"],
    LS: ["Lesotho", "Лесото"],
    LT: ["Lithuania", "Литва", "Литвы", "Литве"],
    LU: ["Luxembourg", "Люксембург"],
    LV: ["Latvia", "Латвия"],
    LY: ["Libya", "Ливия"],
    MA: ["Morocco", "Марокко"],
    MC: ["Monaco", "Монако"],
    MD: ["Moldova", "Молдова"],
    ME: ["Montenegro", "Черногория"],
    MF: ["St. Martin", "Сен-Мартен"],
    MG: ["Madagascar", "Мадагаскар"],
    MH: ["Marshall Islands", "Маршалловы о-ва"],
    MK: ["North Macedonia", "Северная Македония"],
    ML: ["Mali", "Мали"],
    MM: ["Myanmar (Burma)", "Myanmar", "Мьянма (Бирма)"],
    MN: ["Mongolia", "Монголия"],
    MO: ["Macao SAR China", "Macao", "Макао (САР)", "Макао"],
    MP: ["Northern Mariana Islands", "Северные Марианские о-ва"],
    MQ: ["Martinique", "Мартиника"],
    MR: ["Mauritania", "Мавритания"],
    MS: ["Montserrat", "Монтсеррат"],
    MT: ["Malta", "Мальта"],
    MU: ["Mauritius", "Маврикий"],
    MV: ["Maldives", "Мальдивы"],
    MW: ["Malawi", "Малави"],
    MX: ["Mexico", "Мексика"],
    MY: ["Malaysia", "Малайзия"],
    MZ: ["Mozambique", "Мозамбик"],
    NA: ["Namibia", "Намибия"],
    NC: ["New Caledonia", "Новая Каледония"],
    NE: ["Niger", "Нигер"],
    NF: ["Norfolk Island", "о-в Норфолк"],
    NG: ["Nigeria", "Нигерия"],
    NI: ["Nicaragua", "Никарагуа"],
    NL: ["Netherlands", "Нидерланды", "Нидерландов", "Нидерландах"],
    NO: ["Norway", "Норвегия"],
    NP: ["Nepal", "Непал"],
    NR: ["Nauru", "Науру"],
    NU: ["Niue", "Ниуэ"],
    NZ: ["New Zealand", "Aotearoa New Zealand", "Новая Зеландия", "Аотеароа (Новая Зеландия)"],
    OM: ["Oman", "Оман"],
    PA: ["Panama", "Панама"],
    PC: [],
    PE: ["Peru", "Перу"],
    PF: ["French Polynesia", "Французская Полинезия"],
    PG: ["Papua New Guinea", "Папуа — Новая Гвинея"],
    PH: ["Philippines", "Филиппины"],
    PK: ["Pakistan", "Пакистан"],
    PL: ["Poland", "Польша", "Польше", "Польши"],
    PM: ["St. Pierre & Miquelon", "Сен-Пьер и Микелон"],
    PN: ["Pitcairn Islands", "Pitcairn", "о-ва Питкэрн"],
    PR: ["Puerto Rico", "Пуэрто-Рико"],
    PS: ["Palestinian Territories", "Palestine", "Палестинские территории", "Палестина"],
    PT: ["Portugal", "Португалия"],
    PW: ["Palau", "Палау"],
    PY: ["Paraguay", "Парагвай"],
    QA: ["Qatar", "Катар"],
    RE: ["Réunion", "Реюньон"],
    RO: ["Romania", "Румыния", "Румынии"],
    RS: ["Serbia", "Сербия"],
    RU: ["Russia", "Россия", "России"],
    RW: ["Rwanda", "Руанда"],
    SA: ["Saudi Arabia", "Саудовская Аравия"],
    SB: ["Solomon Islands", "Соломоновы о-ва"],
    SC: ["Seychelles", "Сейшельские о-ва"],
    SD: ["Sudan", "Судан"],
    SE: ["Sweden", "Швеция", "Швеции"],
    SG: ["Singapore", "Сингапур", "Сингапуре"],
    SH: ["St. Helena", "о-в Св. Елены"],
    SI: ["Slovenia", "Словения"],
    SJ: ["Svalbard & Jan Mayen", "Шпицберген и Ян-Майен"],
    SK: ["Slovakia", "Словакия"],
    SL: ["Sierra Leone", "Сьерра-Леоне"],
    SM: ["San Marino", "Сан-Марино"],
    SN: ["Senegal", "Сенегал"],
    SO: ["Somalia", "Сомали"],
    SR: ["Suriname", "Суринам"],
    SS: ["South Sudan", "Южный Судан"],
    ST: ["São Tomé & Príncipe", "Сан-Томе и Принсипи"],
    SV: ["El Salvador", "Сальвадор"],
    SX: ["Sint Maarten", "Синт-Мартен"],
    SY: ["Syria", "Сирия"],
    SZ: ["Eswatini", "Swaziland", "Эсватини", "Свазиленд"],
    TC: ["Turks & Caicos Islands", "Тёркс и Кайкос"],
    TD: ["Chad", "Чад"],
    TF: ["French Southern Territories", "Французские Южные территории"],
    TG: ["Togo", "Того"],
    TH: ["Thailand", "Таиланд"],
    TJ: ["Tajikistan", "Таджикистан"],
    TK: ["Tokelau", "Токелау"],
    TL: ["Timor-Leste", "East Timor", "Восточный Тимор", "Тимор-Лесте"],
    TM: ["Turkmenistan", "Туркменистан"],
    TN: ["Tunisia", "Тунис"],
    TO: ["Tonga", "Тонга"],
    TR: ["Türkiye", "Turkey", "Турция", "Турции"],
    TT: ["Trinidad & Tobago", "Тринидад и Тобаго"],
    TV: ["Tuvalu", "Тувалу"],
    TW: ["Taiwan", "Тайвань"],
    TZ: ["Tanzania", "Танзания"],
    UA: ["Ukraine", "Украина"],
    UG: ["Uganda", "Уганда"],
    UM: ["U.S. Outlying Islands", "Внешние малые о-ва (США)"],
    UN: ["United Nations", "Организация Объединенных Наций", "ООН"],
    US: ["United States", "Соединенные Штаты", "США", "USA", "U S A", "United States of America", "America"],
    UY: ["Uruguay", "Уругвай"],
    UZ: ["Uzbekistan", "Узбекистан"],
    VA: ["Vatican City", "Ватикан"],
    VC: ["St. Vincent & Grenadines", "Сент-Винсент и Гренадины"],
    VE: ["Venezuela", "Венесуэла"],
    VG: ["British Virgin Islands", "Виргинские о-ва (Великобритания)"],
    VI: ["U.S. Virgin Islands", "Виргинские о-ва (США)"],
    VN: ["Vietnam", "Вьетнам"],
    VU: ["Vanuatu", "Вануату"],
    WF: ["Wallis & Futuna", "Уоллис и Футуна"],
    WS: ["Samoa", "Самоа"],
    XK: ["Kosovo", "Косово"],
    XX: [],
    YE: ["Yemen", "Йемен"],
    YT: ["Mayotte", "Майотта"],
    ZA: ["South Africa", "Южно-Африканская Республика"],
    ZM: ["Zambia", "Замбия"],
    ZW: ["Zimbabwe", "Зимбабве"]
}

var shortCodes = { USA: "US", UK: "GB", UAE: "AE" }
var nameIndex = buildNameIndex()

function codeFor(value) {
    var code = value.trim().toUpperCase()
    code = shortCodes[code] || code
    return Object.prototype.hasOwnProperty.call(territories, code) ? code : ""
}

function wordsOf(value) {
    return value.toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "")
        .replace(/&/g, " and ").replace(/[^a-zа-я0-9]+/g, " ").trim()
}

function buildNameIndex() {
    var codes = Object.create(null)
    var maxWords = 0
    Object.keys(territories).forEach(function(code) {
        territories[code].forEach(function(name) {
            var words = wordsOf(name)
            codes[words] = codes[words] === undefined || codes[words] === code ? code : ""
            maxWords = Math.max(maxWords, words.split(" ").length)
        })
    })
    return { codes: codes, maxWords: maxWords }
}

function countryForName(name) {
    // Regional-indicator flags are UTF-16 pairs and may occur anywhere in the title.
    var flags = /\uD83C([\uDDE6-\uDDFF])\uD83C([\uDDE6-\uDDFF])/g
    var emoji
    while ((emoji = flags.exec(name)) !== null) {
        var code = codeFor(String.fromCharCode(
            emoji[1].charCodeAt(0) - 0xDDE6 + 65,
            emoji[2].charCodeAt(0) - 0xDDE6 + 65
        ))
        if (code) return code
    }

    var prefix = name.match(/^\s*(?:[[(]\s*)?([A-Za-z]{2,3})(?=$|[^A-Za-zА-Яа-яЁё0-9])/)
    var prefixCode = prefix ? codeFor(prefix[1]) : ""
    if (prefixCode && (prefix[1] === prefix[1].toUpperCase() || /^\s*[[(]/.test(name)))
        return prefixCode

    var words = wordsOf(name).split(" ")
    // Match the full country before shorter names such as Guinea or Samoa.
    for (var count = Math.min(words.length, nameIndex.maxWords); count >= 1; --count) {
        for (var start = 0; start + count <= words.length; ++start) {
            var match = nameIndex.codes[words.slice(start, start + count).join(" ")]
            if (match) return match
        }
    }

    // Keep lowercase prefixes such as "de-02", without treating ordinary words as codes.
    if (prefixCode && !/^(am|as|at|be|by|do|in|is|it|me|my|no|so|to|us|we)$/i.test(prefix[1]))
        return prefixCode
    return ""
}
