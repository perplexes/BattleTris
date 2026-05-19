h61122
s 00000/00000/00000
d R 1.2 01/10/20 13:34:54 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/btref/btsignals.H
c Name history : 1 0 src/btref/btsignals.H
e
s 00030/00000/00000
d D 1.1 01/10/20 13:34:53 bmc 1 0
c date and time created 01/10/20 13:34:53 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef BTSIGNALS_H
#define BTSIGNALS_H

#include "BTConfig.H"

#include <iostream.h>

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "SigHandler.H"

class BTRefTermHandler : public SigHandler {

public:
  BTRefTermHandler(SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp) {}

  virtual ~BTRefTermHandler() {}

  void handle() {
    if(maskint)
      cerr << "btref: Interrupt ignored in critical section" << endl;
    else
      exit(0);
  }
};

#endif
E 1
