вот мой проект на rust и libp2p

реализован тест на python, но из-за работы с процессами и проблемой перехвата вывода, тесты сбоят.

я принял решение что надо сделать библиотеку обертку для python используя pyo3.

требования:
* проект на python должен использовать poetry
* на python должны быть доступны такие классы как PeerID, KeyPair
* Node с реализацией почти всех команд из comander.rs, очередь сообщений NetworkEvent, open_stream возвращает XStream объект, все методы асинхронные
* XStream со всеми асинхронными методами.
* желательно оболочки для python реализовать в файлах py/node.rs py/xstream.rs py/types.rs (для PeerID, KeyPair) и тд
* пример кода на python аналогичный main.rs, где можно из терминала запустить узел, подключиться, провести поиск итд
