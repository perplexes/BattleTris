/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTSDSignals.H                                       */
/*    DATE: Wed Oct  5 17:33:45 1994                            */
/****************************************************************/

#ifndef BTSDSIGNALS_H
#define BTSDSIGNALS_H

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTSlave.H"
#include "SigHandler.H"

class BTSlave;

class BTSDTermHandler : public SigHandler {
private:
  BTSlave *slave_;
public:
  BTSDTermHandler(BTSlave *slave, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), slave_(slave) {}
  virtual ~BTSDTermHandler() {}
  void handle() {
    slave_->terminate();
    exit(0);
  }
};

class BTSDLst1Handler : public SigHandler {
private:
  BTSlave *slave_;
public:
  BTSDLst1Handler(BTSlave *slave, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), slave_(slave) {}
  virtual ~BTSDLst1Handler() {}
  void handle() {
    slave_->listClients();
  }
};

class BTSDLst2Handler : public SigHandler {
private:
  BTSlave *slave_;
public:
  BTSDLst2Handler(BTSlave *slave, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), slave_(slave) {}
  virtual ~BTSDLst2Handler() {}
  void handle() {
    slave_->listQueue();
  }
};

class BTSDPipeHandler : public SigHandler {
private:
  BTSlave *slave_;
public:
  BTSDPipeHandler(BTSlave *slave, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), slave_(slave) {}
  virtual ~BTSDPipeHandler() {}
  void handle() {
    slave_->disconnect();
  }
};

class BTSDHangHandler : public SigHandler {
private:
  BTSlave *slave_;
public:
  BTSDHangHandler(BTSlave *slave, SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), slave_(slave) {}
  virtual ~BTSDHangHandler() {}
  void handle() {
    slave_->restart();
  }
};

#endif
