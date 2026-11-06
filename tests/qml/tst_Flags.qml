import QtQuick
import QtTest
import "../../qml" as Client

Item {
    width: 60
    height: 40

    Client.Flag { id: flag }

    TestCase {
        name: "CountryFlags"

        function test_country_data() {
            return [
                { tag: "Lithuania", title: "Lithuania Auto", expected: "LT" },
                { tag: "Romania", title: "Romania Auto", expected: "RO" },
                { tag: "USA", title: "USA Auto", expected: "US" },
                { tag: "Germany", title: "Germany Auto", expected: "DE" },
                { tag: "Netherlands", title: "Netherlands Auto", expected: "NL" },
                { tag: "Sweden", title: "Sweden Auto", expected: "SE" },
                { tag: "Russian", title: "Румыния — резерв", expected: "RO" },
                { tag: "Russian inflection", title: "Сервер в Литве", expected: "LT" },
                { tag: "code prefix", title: "RO-02", expected: "RO" },
                { tag: "lowercase code", title: "de-02", expected: "DE" },
                { tag: "bracketed code", title: "[lt] Auto", expected: "LT" },
                { tag: "longest name", title: "Papua New Guinea Auto", expected: "PG" },
                { tag: "accented name", title: "Curaçao Auto", expected: "CW" },
                { tag: "territory", title: "Hong Kong Auto", expected: "HK" },
                { tag: "emoji priority", title: "VPN 🇷🇴 Lithuania Auto", expected: "RO" },
                { tag: "EU emoji", title: "🇪🇺 LTE обход БС 1 🟡", expected: "EU" },
                { tag: "unknown emoji", title: "🇿🇿 Romania Auto", expected: "RO" },
                { tag: "unknown location", title: "LTE обход БС 1 🟡", expected: "" },
                { tag: "plain words", title: "In service", expected: "" },
                { tag: "whole word", title: "Americana Auto", expected: "" },
                { tag: "explicit code", title: "🇷🇴 Romania", code: "lt", expected: "LT" }
            ]
        }

        function test_country(row) {
            flag.code = row.code || ""
            flag.name = row.title
            compare(flag.resolvedCode, row.expected)
            var image = findChild(flag, "countryFlagImage")
            verify(image !== null)
            tryCompare(image, "status", Image.Ready)
            if (row.expected)
                verify(String(image.source).endsWith("/flags/" + row.expected.toLowerCase() + ".svg"))
            else
                verify(String(image.source).startsWith("data:image/svg+xml"))
        }
    }
}
