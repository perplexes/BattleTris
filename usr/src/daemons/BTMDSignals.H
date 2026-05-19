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
