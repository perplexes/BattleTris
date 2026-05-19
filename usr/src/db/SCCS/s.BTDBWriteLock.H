h32298
s 00000/00000/00000
d R 1.2 01/10/20 13:34:49 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTDBWriteLock.H
c Name history : 1 0 src/db/BTDBWriteLock.H
e
s 00044/00000/00000
d D 1.1 01/10/20 13:34:48 bmc 1 0
c CodeManager Uniquification: src/db/BTDBWriteLock.H
c date and time created 01/10/20 13:34:48 by bmc
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
/*    FILE: BTDBWriteLock.H                                     */
/*    DATE: Sat Sep  3 21:52:09 1994                            */
/****************************************************************/

#ifndef BTDBWRITELOCK_H
#define BTDBWRITELOCK_H

#include "BTConfig.H"
#include "BTDBLock.H"

class BTDBWriteLock : public BTDBLock {

public:
  BTDBWriteLock(int filedes, off_t offset = 0, off_t length = 0)
  : BTDBLock(filedes, offset, length) {}

  short lock() {
    short err = lockRegion(filedes_, F_SETLK, F_WRLCK, offset_, length_);

    if(err == ERRBTDBNOERR)
      locked_ = 1;

    return err;
  }

  short lockw() {
    short err = lockRegion(filedes_, F_SETLKW, F_WRLCK, offset_, length_);

    if(err == ERRBTDBNOERR)
      locked_ = 1;

    return err;
  }

  pid_t testLock() {
    return testRegion(filedes_, F_WRLCK, offset_, length_);
  }

};

#endif
E 1
