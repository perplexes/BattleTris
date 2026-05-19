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
