h08778
s 00000/00000/00000
d R 1.2 01/10/20 13:34:57 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/daemons/BTSlave.H
c Name history : 1 0 src/daemons/BTSlave.H
e
s 00083/00000/00000
d D 1.1 01/10/20 13:34:56 bmc 1 0
c CodeManager Uniquification: src/daemons/BTSlave.H
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
/*    FILE: BTSlave.H                                           */
/*    DATE: Wed Oct  5 01:04:50 1994                            */
/****************************************************************/

#ifndef BTSLAVE_H
#define BTSLAVE_H

#include "BTConfig.H"

#include <fstream.h>

#include "PacketBuffer.H"

class StreamSocket;
class BTDBServer;
class BTSDClient;

enum {
  BTSD_MAXCLIENTS = 10,

  BTSD_CTMOUT_SECS = 60,
  BTSD_CTMOUT_USECS = 0,

  BTSD_TMOUT_SECS = 30,
  BTSD_TMOUT_USECS = 0,

  BTSD_POLL_SECS = 30,
  BTSD_POLL_USECS = 0
};

class BTSlave {

friend class BTSDTermHandler;
friend class BTSDLst1Handler;
friend class BTSDLst2Handler;
friend class BTSDPipeHandler;
friend class BTSDHangHandler;

private:
  StreamSocket *master_;
  PacketBuffer pbuf_;
  PacketBuffer cbuf_;

  int maxfd_;
  int awol_;

  BTSDClient **clients_;
  int maxclients_;

  BTDBServer *dbserver_;
  ofstream log_;

  timeval timeout_;
  short err_;

private:
  void process(int idx);

  void listClients();
  void listQueue();
  void restart();

public:
  BTSlave(const char *logpath, int prindex, int maxclients = BTSD_MAXCLIENTS);
  ~BTSlave() { BTSlave::terminate(); }

  void disconnect();
  void terminate();

  short obey();
  short run();

  short err() { return err_; }

private:
  BTSlave(const BTSlave& other);
  BTSlave& operator=(const BTSlave& other);
};

#endif
E 1
