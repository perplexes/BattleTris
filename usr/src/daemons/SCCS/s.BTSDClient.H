h52717
s 00000/00000/00000
d R 1.2 01/10/20 13:34:57 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/daemons/BTSDClient.H
c Name history : 1 0 src/daemons/BTSDClient.H
e
s 00054/00000/00000
d D 1.1 01/10/20 13:34:56 bmc 1 0
c date and time created 01/10/20 13:34:56 by bmc
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
/*    FILE: BTSDClient.H                                        */
/*    DATE: Wed Oct  5 14:00:42 1994                            */
/****************************************************************/

#ifndef BTSDCLIENT_H
#define BTSDCLIENT_H

#include "BTConfig.H"

#include <iostream.h>

#include "StreamSocket.H"
#include "PacketBuffer.H"

#include "BTConstants.H"
#include "BTProtocol.H"
#include "BTNetworkEntry.H"

class BTSDClient {

friend inline ostream& operator<<(ostream& os, BTSDClient& client);

private:
  BTNetworkEntry entry_;
  PacketBuffer pbuf_;
  StreamSocket *socket_;
  short err_;

public:
  BTSDClient(StreamSocket *socket);
  ~BTSDClient();

  BTNetworkEntry *entry() { return &entry_; }
  StreamSocket *sock() { return socket_; }
  int sockfd() { return socket_->sock(); }
  short error() { return err_; }

private:
  BTSDClient(const BTSDClient& other);
  BTSDClient& operator=(const BTSDClient& other);
};

ostream& operator<<(ostream& os, BTSDClient& client)
{
  os << client.entry_.userName_ << '@' << client.entry_.hostName_
     << '.' << client.entry_.port_;

  return os;
}

#endif
E 1
