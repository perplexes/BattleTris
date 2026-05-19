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
