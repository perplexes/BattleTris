/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTMDSlave.C                                         */
/*    DATE: Mon Oct  3 00:51:42 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include <signal.h>
#include <stdio.h>

#include "StreamSocket.H"
#include "StreamSocketErr.H"

#include "BTDirs.H"
#include "BTConfigFile.H"
#include "BTProtocol.H"
#include "BTMDSlave.H"
#include "btserverd.H"

const char *BTMD_SLAVE_ARGV0 = "btslaved";
const char *BTMD_SLAVE_ARGV1 = "-i";
const char *BTMD_SLAVE_ARGV3 = "-f";

const int BTMD_TMOUT_SECS = 30;
const int BTMD_TMOUT_USECS = 0;

BTMDSlave::BTMDSlave(StreamSocket *sock, int prindex)
: prindex_(prindex), err_(0)
{
  char buf[16];

  switch(pid_ = fork()) {

  case -1:
    err_ = 1;
    return;

  case 0:
    sprintf(buf, "%d", prindex);
    if(execl(g_conf->slvpath(), BTMD_SLAVE_ARGV0, BTMD_SLAVE_ARGV1,
	     buf, BTMD_SLAVE_ARGV3, configfile, (char *) 0) < 0)
      exit(1);

  default:
    timeout_.tv_sec = BTMD_TMOUT_SECS;
    timeout_.tv_usec = BTMD_TMOUT_USECS;

    if(sock->ready(timeout_)) {
      if((err_ = sock->accept(slaveSock_)) < 0)
	return;

      pbuf_.socket(slaveSock_);

      if((err_ = pbuf_.sendpacket(BT_OBEY_ME)) < 0)
	return;

      if((err_ = pbuf_.recvpacket()) < 0)
	return;
      
      if(pbuf_.datatype() != BT_I_OBEY) {
	err_ = 1;
	return;
      }
    } else {
      err_ = 1;
      return;
    }
  }
}

BTMDSlave::~BTMDSlave()
{
  if(err_ != 0)
    return;

  pbuf_.sendpacket(BT_HARIKARI);
  delete slaveSock_;
  kill(pid_, SIGTERM);
}

short BTMDSlave::acceptClient(StreamSocket *client)
{
  short err;

  if((err = pbuf_.sendpacket(BT_NEWCLIENT)) < 0)
    return err;

  if((err = pbuf_.recvpacket()) < 0)
    return err;

  if(pbuf_.datatype() == BT_CLIENTOK) {
    if((err = slaveSock_->sendfd(client->sock())) < 0)
      return err;
    return 1;
  }

  return 0;
}

ostream& operator<<(ostream& os, BTMDSlave& slave)
{
  return os << "btslaved: process " << slave.pid() << endl;
}
