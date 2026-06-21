document.addEventListener('DOMContentLoaded', () => {
    const button = document.getElementById('action-button');
    const message = document.getElementById('message');

    if (!button || !message) {
        return;
    }

    button.addEventListener('click', () => {
        const now = new Date().toLocaleTimeString('es-ES');
        message.textContent = `¡JavaScript funciona! Hora local: ${now}`;
        message.classList.remove('hidden');
    });
});
