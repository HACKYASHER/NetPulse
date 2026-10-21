export const palettes = [
  { name: "Фиолетовый", en: "Violet", accent: "#A78BFA", surface: "#171325" },
  { name: "Океан", en: "Ocean", accent: "#38BDF8", surface: "#102332" },
  { name: "Лес", en: "Forest", accent: "#34D399", surface: "#10251E" },
  { name: "Графит", en: "Graphite", accent: "#CBD5E1", surface: "#20242B" },
  { name: "Рассвет", en: "Dawn", accent: "#F59E0B", surface: "#FFF4E6" },
];
export function surfaceColors(hex: string) {
  const rgb = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  const light = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722 > 150;
  const mix = (value: number, part: number) =>
    "#" +
    rgb
      .map((n) =>
        Math.round(n * (1 - part) + value * part)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("");
  return {
    light,
    "--surface": hex,
    "--bg": mix(light ? 255 : 0, light ? 0.35 : 0.45),
    "--card": mix(light ? 0 : 255, light ? 0.035 : 0.045),
    "--border": mix(light ? 0 : 255, 0.18),
    "--text": light ? "#171A28" : "#F5F6FA",
    "--muted": light ? "#535C70" : "#ABB3C4",
  };
}
