h06255
s 00000/00000/00000
d R 1.2 01/10/20 13:35:35 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTStopwatch.H
c Name history : 1 0 src/game/BTStopwatch.H
e
s 00026/00000/00000
d D 1.1 01/10/20 13:35:34 bmc 1 0
c date and time created 01/10/20 13:35:34 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
