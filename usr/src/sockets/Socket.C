/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: Socket.C                                            */
/*    DATE: Fri Apr 15 22:20:54 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "Socket.H"

Socket::Socket(int sock)
: sock_(sock)
{
  for(int i = 0; i < SOCKET_CB_NREASONS; i++)
    callbacks_[i] = 0;
}

Socket::~Socket()
{
  for(int i = 0; i < SOCKET_CB_NREASONS; i++)
    Socket::removeCB((SocketCBReason) i);
}

void Socket::installCB(SocketCB *cb, SocketCBReason reason)
{
  Socket::removeCB(reason);

  SocketCBData *cbdata = new SocketCBData;
  cbdata->reason_ = reason;
  cbdata->sock_ = this;
  cbdata->cb_ = cb;

  callbacks_[reason] = cbdata;
  cb->install(cbdata);
}
