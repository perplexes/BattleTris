h17495
s 00000/00000/00000
d R 1.2 01/10/20 13:35:00 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/Socket.C
c Name history : 1 0 src/sockets/Socket.C
e
s 00035/00000/00000
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
E 1
