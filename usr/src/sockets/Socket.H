/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: Socket.H                                            */
/*    DATE: Mon Feb  7 15:32:25 1994                            */
/****************************************************************/

#ifndef SOCKET_H
#define SOCKET_H

#include "BTConfig.H"

#include <sys/time.h>

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
