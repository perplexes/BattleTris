h44243
s 00000/00000/00000
d R 1.2 01/10/20 13:34:55 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/daemons/BTMDSignals.H
c Name history : 1 0 src/daemons/BTMDSignals.H
e
s 00083/00000/00000
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
/*    FILE: BTMDSignals.H                                       */
/*    DATE: Wed Sep 28 15:19:07 1994                            */
/****************************************************************/

#ifndef BTMDSIGNALS_H
#define BTMDSIGNALS_H

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTServer.H"
#include "SigHandler.H"

class BTServer;

class BTMDTermHandler : public SigHandler {
private:
  BTServer *server_;
public:
  BTMDTermHandler(BTServer *server, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), server_(server) {}
  virtual ~BTMDTermHandler() {}
  void handle() {
    server_->killSlaves();
    exit(0);
  }
};

class BTMDListHandler : public SigHandler {
private:
  BTServer *server_;
public:
  BTMDListHandler(BTServer *server, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), server_(server) {}
  virtual ~BTMDListHandler() {}
  void handle() {
    server_->listSlaves();
  }
};

class BTMDSuspHandler : public SigHandler {
private:
  BTServer *server_;
public:
  BTMDSuspHandler(BTServer *server, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), server_(server) {}
  virtual ~BTMDSuspHandler() {}
  void handle() {
    server_->toggleListener();
  }
};

class BTMDChldHandler : public SigHandler {
private:
  BTServer *server_;
public:
  BTMDChldHandler(BTServer *server, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), server_(server) {}
  virtual ~BTMDChldHandler() {}
  void handle() {
    server_->deadSlave();
  }
};

class BTMDHangHandler : public SigHandler {
private:
  BTServer *server_;
public:
  BTMDHangHandler(BTServer *server, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), server_(server) {}
  virtual ~BTMDHangHandler() {}
  void handle() {
    server_->restartSlaves();
  }
};

#endif
E 1
