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
