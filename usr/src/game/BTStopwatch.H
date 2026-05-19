/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTStopwatch.H                                       */
/*    ASSN:                                                     */
/*    DATE: Sun May  8 05:23:49 1994                            */
/****************************************************************/

#ifndef BTSTOPWATCH_H
#define BTSTOPWATCH_H

#include "BTConfig.H"

class BTStopwatch {
private:
  time_t start_, total_;
public:
  BTStopwatch() : total_ (0) {};
  void restart() { start_ = time(0); total_ = 0; }
  void stop() { total_ += time(0) - start_; }
  void start() { start_ = time(0); }
  time_t elapsed() { return total_; }
};


#endif
