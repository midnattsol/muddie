# A MUD of the tests' own: it sends every client each line appended to
# mud.txt in the folder given, and keeps what clients send in mud.sent.
import os
import socket
import sys
import threading
import time

folder = sys.argv[1]
lines = os.path.join(folder, "mud.txt")
open(lines, "a").close()
server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", 47777))
server.listen(5)


def serve(conn):
    def read():
        with open(os.path.join(folder, "mud.sent"), "ab") as sent:
            while data := conn.recv(4096):
                sent.write(data)
                sent.flush()

    threading.Thread(target=read, daemon=True).start()
    with open(lines, "rb") as source:
        source.seek(0, 2)
        try:
            while True:
                line = source.readline()
                if not line:
                    time.sleep(0.05)
                    continue
                conn.sendall(line.rstrip(b"\n") + b"\r\n")
        except OSError:
            pass


while True:
    conn, _ = server.accept()
    threading.Thread(target=serve, args=(conn,), daemon=True).start()
