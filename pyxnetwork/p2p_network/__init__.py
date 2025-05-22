# p2p_network/__init__.py
from pyxnetwork import (
    Node, 
    PeerId, 
    KeyPair, 
    generate_keypair, 
    peer_id_from_keypair, 
    ProofOfRepresentation,
    # XStream classes
    XStream,
    StreamDirection,
    StreamState,
    PyXStreamError as XStreamError,  # Используем правильное имя класса
    PyErrorOnRead as ErrorOnRead,     # Используем правильное имя класса
    XStreamErrorUtils,                # Добавляем утилиты
)

__version__ = "0.1.0"

__all__ = [
    "Node",
    "PeerId", 
    "KeyPair",
    "generate_keypair",
    "peer_id_from_keypair", 
    "ProofOfRepresentation",
    # XStream exports
    "XStream",
    "StreamDirection",
    "StreamState", 
    "XStreamError",
    "ErrorOnRead",
    "XStreamErrorUtils",
]