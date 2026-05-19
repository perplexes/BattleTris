h54632
s 00000/00000/00000
d R 1.2 01/10/20 13:34:55 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/daemons/BTServer.H
c Name history : 1 0 src/daemons/BTServer.H
e
s 00068/00000/00000
d D 1.1 01/10/20 13:34:54 bmc 1 0
c date and time created 01/10/20 13:34:54 by bmc
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
/*    FILE: BTServer.H                                          */
/*    DATE: Tue Sep 27 23:22:17 1994                            */
/****************************************************************/

#ifndef BTSERVER_H
#define BTSERVER_H

#include "BTConfig.H"

#include <fstream.h>

#include "BTConstants.H"
#include "PacketBuffer.H"

class StreamSocket;
class BTMDSlave;

class BTServer {

friend class BTMDTermHandler;
friend class BTMDListHandler;
friend class BTMDSuspHandler;
friend class BTMDChldHandler;
friend class BTMDHangHandler;

private:
  int death_;
  int sidx_;

  StreamSocket **sockets_;
  BTMDSlave **slaves_;
  int nslaves_;

  StreamSocket *listener_;
  PacketBuffer pbuf_;

  ofstream *log_;
  char *pidfile_;

  int enabled_;
  short err_;

private:
  void toggleListener();
  void restartSlaves();
  void listSlaves();
  void killSlaves();

  void spawnSlave(int prindex);
  void deadSlave();

  int acceptClient(StreamSocket *client);

public:
  BTServer(int nslaves, int port);
  ~BTServer();

  short run();

private:
  BTServer(const BTServer& other);
  BTServer& operator=(const BTServer& other);
};

#endif
E 1
