h58686
s 00000/00000/00000
d R 1.2 01/10/20 13:35:00 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/Socket.H
c Name history : 1 0 src/sockets/Socket.H
e
s 00055/00000/00000
d D 1.1 01/10/20 13:34:59 bmc 1 0
c date and time created 01/10/20 13:34:59 by bmc
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
/*    FILE: Socket.H                                            */
/*    DATE: Mon Feb  7 15:32:25 1994                            */
/****************************************************************/

#ifndef SOCKET_H
#define SOCKET_H

#include "BTConfig.H"

#include "SocketCB.H"

class Address;

class Socket {

protected:
  SocketCBData *callbacks_[SOCKET_CB_NREASONS];
  int sock_;

protected:
  Socket(int sock);

public:
  virtual ~Socket();

  virtual short sendbuf(char *buf, int buflen, Address *dst = 0) = 0;

  virtual short recvbuf(char *buf, int buflen, Address *src = 0) = 0;
  virtual short peekbuf(char *buf, int buflen, Address *src = 0) = 0;

  virtual short recvbuf(char *buf, int buflen, timeval& delay,
		       Address *src = 0) = 0;
  virtual short peekbuf(char *buf, int buflen, timeval& delay,
		       Address *src = 0) = 0;

  virtual int ready(timeval& delay) = 0;
  virtual int ready() = 0;

  void installCB(SocketCB *cb, SocketCBReason reason);

  void removeCB(SocketCBReason reason) {
    if(callbacks_[reason]) {
      (callbacks_[reason])->cb_->remove(callbacks_[reason]);
      delete callbacks_[reason];
      callbacks_[reason] = 0;
    }
  }

  int sock() const { return sock_; }
};

#endif
E 1
