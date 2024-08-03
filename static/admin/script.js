document.body.addEventListener('htmx:configRequest', function (evt) {
    switch (evt.detail.path) {
        case '/chat_log':
            evt.detail.parameters['offset'] = new Date().getTimezoneOffset();
            break;
        default:
            break;
    }
});