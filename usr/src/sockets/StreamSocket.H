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
