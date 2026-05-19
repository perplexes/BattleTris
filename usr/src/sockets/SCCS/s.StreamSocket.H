h32778
s 00000/00000/00000
d R 1.2 01/10/20 13:35:01 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/StreamSocket.H
c Name history : 1 0 src/sockets/StreamSocket.H
e
s 00066/00000/00000
d D 1.1 01/10/20 13:35:00 bmc 1 0
c date and time created 01/10/20 13:35:00 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: StreamSocket.H                                      */
/*    DATE: Sat Apr 16 23:26:32 1994                            */
/****************************************************************/

#ifndef STREAMSOCKET_H
#define STREAMSOCKET_H

#include "BTConfig.H"
#include "StreamSocketErr.H"
#include "Address.H"
#include "Socket.H"

class StreamSocket : public Socket {
private:
  StreamSocket(const InetAddress& clientAddr, int clientSock,
	       const sockaddr_in& peerAddr);

  StreamSocket(const UnixAddress& clientAddr, int clientSock,
	       const sockaddr_un& peerAddr);

protected:
  InetAddress in_addr_;
  InetAddress *in_peer_;

  UnixAddress un_addr_;
  UnixAddress *un_peer_;

public:
  StreamSocket(InetAddress& addr);
  StreamSocket(UnixAddress& addr);

  ~StreamSocket();

  short connect(InetAddress& peer);
  short connect(InetAddress& peer, InetAddress& addr);

  short connect(UnixAddress& peer);
  short connect(UnixAddress& peer, UnixAddress& addr);

  short listen(int backlog);
  short listen(int backlog, InetAddress& addr);
  short listen(int backlog, UnixAddress& addr);

  short accept(StreamSocket *& sockptr);

  short sendbuf(char *buf, int buflen, Address *dst = 0);

  short recvbuf(char *buf, int buflen, Address *src = 0);
  short peekbuf(char *buf, int buflen, Address *src = 0);

  short recvbuf(char *buf, int buflen, timeval& delay, Address *src = 0);
  short peekbuf(char *buf, int buflen, timeval& delay, Address *src = 0);

  int ready();
  int ready(timeval& delay);

  short sendfd(int fd);

  short recvsock_in(StreamSocket *& sockptr);
  short recvfd(int& filedes);
};

#endif
E 1
