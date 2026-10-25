.pragma library

// Keep the engine's original line and severity. These hints explain the failure;
// they never turn a failed request into a successful one or disable TLS checks.
function hint(message, russian) {
    var text=String(message).toLowerCase()
    function tr(ru,en) { return russian?ru:en }
    if (text.indexOf("unreachable network")>=0 || text.indexOf("network is unreachable")>=0)
        return tr("Нет маршрута до адреса. Для IPv6 проверьте поддержку IPv6 у провайдера и выбранный прямой маршрут.","No route to this address. For IPv6, check ISP IPv6 support and the selected direct route.")
    if (text.indexOf("nxdomain")>=0)
        return tr("DNS сообщает, что домен не существует. Домен .invalid специально зарезервирован для недействительных адресов.","DNS reports that the domain does not exist. The .invalid suffix is reserved for invalid addresses.")
    if (text.indexOf("bad record mac")>=0)
        return tr("Нарушена целостность TLS-потока DNS. Проверьте стабильность канала, MTU и DNS-сервер в Сети → DNS; проверка TLS остаётся включённой.","DNS TLS record integrity failed. Check connection stability, MTU and Network → DNS; TLS verification remains enabled.")
    if (text.indexOf("servfail")>=0)
        return tr("DNS-сервер не смог обработать запрос. Проверьте доступность выбранного DNS и маршрут к нему в Сети → DNS.","The DNS resolver could not process the query. Check the selected resolver and its route in Network → DNS.")
    if (text.indexOf("clientconn.close")>=0 || text.indexOf("unexpected eof")>=0 || (text.indexOf("dns packet")>=0 && text.indexOf("canceled by remote")>=0))
        return tr("DNS-соединение прервано. Если ошибка повторяется без переключения сервера, проверьте доступность DNS и стабильность канала.","The DNS connection was interrupted. If this repeats without switching servers, check DNS reachability and connection stability.")
    if (text.indexOf("i/o timeout")>=0)
        return tr("Адрес не ответил вовремя. outbound/direct означает прямое соединение по вашим правилам; проверьте доступность адреса и маршрут.","The address did not respond in time. outbound/direct means a direct connection under your rules; check reachability and routing.")
    if (text.indexOf("only one usage of each socket")>=0 || text.indexOf("address already in use")>=0)
        return tr("Локальный порт занят. Измените его в Сети → Подключение или завершите использующее порт приложение.","The local port is busy. Change it in Network → Connection or close the application using it.")
    return ""
}
