/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTLine.H                                            */
/*    ASSN:                                                     */
/*    DATE: Wed Apr 13 17:01:31 1994                            */
/****************************************************************/

#ifndef BTLINE_H
#define BTLINE_H

#include "BTConfig.H"

class BTLine {
private:
  int increment_;
  
public:
  BTLine (int total = 0) : increment_ (0) {}
  void operator++ () { increment_++; }
  int operator== (int other) { 
    if (other == increment_) return 1;
    else return 0;
  }	
  int operator!() {
    if (increment_ == 0) return 1;
    else return 0;
  }
  int inc() { return increment_; }
};

#endif
