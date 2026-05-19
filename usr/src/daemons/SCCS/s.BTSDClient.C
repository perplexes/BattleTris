h03127
s 00000/00000/00000
d R 1.2 01/10/20 13:34:57 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/daemons/BTSDClient.C
c Name history : 1 0 src/daemons/BTSDClient.C
e
s 00025/00000/00000
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
/*    FILE: BTSDClient.C                                        */
/*    DATE: Wed Oct  5 14:19:28 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTProtocol.H"
#include "BTSDClient.H"

BTSDClient::BTSDClient(StreamSocket *socket)
: socket_(socket), pbuf_(socket), err_(0)
{
  if((err_ = pbuf_.recvpacket()) < 0)
    return;

  entry_.readbuf(pbuf_.databuf());
}

BTSDClient::~BTSDClient()
{
  pbuf_.sendpacket(BT_DISCONNECT);
  delete socket_;
}
E 1
