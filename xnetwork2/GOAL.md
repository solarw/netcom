Цель проекта предоставить высокоуровненвый апи где можно с легкостью работать с разными узлами даже за натом или в труднодоступных сетях


упрощенный пример целевого апи:

node.start()
node.listen()
node.bootstrap('some predefined bootstrap node or nodes multi addr')
node.set_xauth_mode(allow_all) # 



stream = node.do_xstream(peer_id).await (searches, connects and opens stream automatically)

для большего конттроля

stream = node.do_xstream(pee_id).with_timeout(d60 secons).with_multiaddr(some_multi_addr).await



конечная цель стартовать  ноду "войти в сеть" через bootstrap
и начать работать с любым пиров в сети просто открывая потоки

главное: минимальная настройка, и максимальная простота, но при желании можно настраивать очень тонко вручную

