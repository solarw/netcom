Добавить pending_streams.rs

с объектом PendingStreamsManager:
работает с libp2p:Streams, и составляет из них SubStreamsPair key: {key: SubstreamKey, main: libp2p::stream, error: libp2p::Stream}

реализует функцию handle_substream(stream, direction, peer_id, connection_id) в которой читает header с read_header. откуда получает XStreamID и SubstreamRole.
составляет ключ. SubstreamKey{direction, peer_id, connection_id, xstreamid} если по этому ключу в списке pendng stream есть поток, то извлекаем его (удаляя) из кеша, проверяем его роль, она должна отличаться от роли обрабатываемого потока, если это так
то из двух потоков с разными ролями мы составляет SubStreamsPair. если вдруг роли совпадают, то закрываем оба потока (подавляя ошибки но выводя их в error) и возвращаем ошибку SameRoleStream.
так же вероятен случай, что один поток был открыт а второй нет, в этом случае работает таймаут 15 секунд (настраиваемый), и непарный поток удаляется из кеша, генерируя сообщение SubstreamTimeoutError

наверное лучший вариант построить систему на сообщениях:
где результат выдается как сообщение:
SubStreamPairReady
SubstreamError{
    SubStreamTimeoutError
    SubStreamSameRole
    SubStreamReadHeaderError
}

PendingStreamsManager не создает и вообще никак не работает с XStream он принимает libp2p потоки, и составляет из них пару на основе SubstreamKey и роли потоков

pending stream manager будет встроен в behaviour в одном экземпляре
у него будет входная очередь событий, которая принимает NewStream
и очередь сообщений к behaviour где будет сообщать об открытии пары SubStreamsPair или ошибках.


все работы с heaer такие как чтение и запись внеусути в отдельный файл

предоставь только pending_streams.rs и header.rs

pendingstreammanager должен подключаться к behaviour через poll
то есть behaviour своем цикле poll иногда передает управление pending manager, где он обрабатывает события




TODO LIST: 
* beh.open_stream добавить поддержку указания connection_id


