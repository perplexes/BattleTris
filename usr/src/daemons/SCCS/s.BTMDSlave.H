h31757
s 00000/00000/00000
d R 1.2 01/10/20 13:34:55 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/daemons/BTMDSlave.H
c Name history : 1 0 src/daemons/BTMDSlave.H
e
s 00049/00000/00000
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
/*    FILE: BTMDSlave.H                                         */
/*    DATE: Mon Oct  3 00:44:02 1994                            */
/****************************************************************/

#ifndef BTMDSLAVE_H
#define BTMDSLAVE_H

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include <iostream.h>

#include "PacketBuffer.H"

class StreamSocket;

class BTMDSlave {

friend ostream& operator<<(ostream& os, BTMDSlave& slave);

private:
  PacketBuffer pbuf_;
  StreamSocket *slaveSock_;
  timeval timeout_;
  int prindex_;
  pid_t pid_;
  short err_;

public:
  BTMDSlave(StreamSocket *sock, int prindex = 0);
  ~BTMDSlave();

  short acceptClient(StreamSocket *client);

  short error() { return err_; }
  pid_t pid() { return pid_; }

private:
  BTMDSlave(const BTMDSlave& other);
  BTMDSlave& operator=(const BTMDSlave& other);
};

#endif
E 1
