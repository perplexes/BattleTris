h60930
s 00000/00000/00000
d R 1.2 01/10/20 13:34:59 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/SocketCB.H
c Name history : 1 0 src/sockets/SocketCB.H
e
s 00066/00000/00000
d D 1.1 01/10/20 13:34:58 bmc 1 0
c date and time created 01/10/20 13:34:58 by bmc
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
/*    FILE: SocketCB.H                                          */
/*    DATE: Sat Dec  3 12:02:53 1994                            */
/****************************************************************/

#ifndef SOCKETCB_H
#define SOCKETCB_H

#include "BTConfig.H"

class Socket;
class SocketCB;

typedef void (*socketcb_t)(Socket *sock, void *data);

enum SocketCBReason {
  SOCKET_CB_READ,
  SOCKET_CB_WRITE,
  SOCKET_CB_EXCEPT,
  SOCKET_CB_NREASONS
};

struct SocketCBData {
  SocketCBReason reason_;
  Socket *sock_;
  SocketCB *cb_;
  long data_;
};

class SocketCB {

friend class Socket;

protected:
  socketcb_t callback_;
  void *data_;

protected:
  virtual void install(SocketCBData *cbdata) = 0;
  virtual void remove(SocketCBData *cbdata) = 0;

public:
  SocketCB(socketcb_t callback, void *data)
  : callback_(callback), data_(data) {}

  SocketCB(const SocketCB& other)
  : callback_(other.callback_), data_(other.data_) {}

  virtual ~SocketCB() {}

  void docall(SocketCBData *cbdata) {
    (cbdata->cb_->callback_)(cbdata->sock_, cbdata->cb_->data_);
  }

  SocketCB& operator=(const SocketCB& other) {
    if(this == &other)
      return *this;
    callback_ = other.callback_;
    data_ = other.data_;
    return *this;
  }
};

#endif
E 1
