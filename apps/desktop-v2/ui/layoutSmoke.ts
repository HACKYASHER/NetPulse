// Runs only inside the isolated packaged WebView smoke mode, never a browser server.
export function checkLayout() {
  const rect = (e: Element) => e.getBoundingClientRect();
  for (const search of document.querySelectorAll(".search")) {
    const input = search.querySelector("input"),
      help = search.querySelector(".setting-help"),
      icon = search.querySelector(":scope > svg");
    if (!input || !help || !icon) continue;
    if (
      rect(input).width < rect(search).width * 0.6 ||
      rect(icon).right > rect(input).left + 1 ||
      rect(input).right > rect(help).left + 1 ||
      rect(help).right > rect(search).right - 3
    )
      throw Error("Search field or helper overlaps");
  }
  for (const menu of document.querySelectorAll(".view-menu")) {
    const select = menu.querySelector("select"),
      icons = menu.querySelectorAll("svg");
    if (!select || icons.length !== 2)
      throw Error("Display picker is incomplete");
    const style = getComputedStyle(select),
      bounds = rect(select);
    if (
      rect(icons[0]).right > bounds.left + parseFloat(style.paddingLeft) ||
      rect(icons[1]).left < bounds.right - parseFloat(style.paddingRight)
    )
      throw Error("Display picker icons overlap its text");
  }
  for (const textarea of document.querySelectorAll(
    ".import-key-field textarea",
  )) {
    if (rect(textarea).width < rect(textarea.closest("label")!).width * 0.95)
      throw Error("Import key field is compressed");
  }
  for (const row of document.querySelectorAll(".application-row")) {
    if (row.querySelector(".setting-help"))
      throw Error("Per-EXE helper must not be rendered");
    const box = row.querySelector("input"),
      text = row.querySelector(":scope > span:last-child");
    if (
      box &&
      text &&
      (rect(box).bottom < rect(text).top || rect(box).top > rect(text).bottom)
    )
      throw Error("Application checkbox wrapped to a separate row");
  }
  for (const selector of document.querySelectorAll(".core-selector")) {
    if (
      selector.querySelectorAll("button").length !== 3 ||
      selector.querySelectorAll('[aria-pressed="true"]').length !== 1
    )
      throw Error("Core selection state is missing");
    for (const button of selector.querySelectorAll("button")) {
      if (
        rect(button).height > 38 ||
        button.scrollWidth > button.clientWidth + 1
      )
        throw Error("Core buttons must stay compact and unwrapped");
    }
    if (rect(selector).width > 200) throw Error("Core selector is too wide");
  }
  for (const interval of document.querySelectorAll(".playground-interval")) {
    const caption = interval.querySelector(".field-caption"),
      select = interval.querySelector("select");
    if (
      caption &&
      select &&
      rect(select).width > 0 &&
      Math.abs(
        (rect(caption).top +
          rect(caption).bottom -
          rect(select).top -
          rect(select).bottom) /
          2,
      ) > 1
    )
      throw Error("Playground interval is not aligned");
  }
}
