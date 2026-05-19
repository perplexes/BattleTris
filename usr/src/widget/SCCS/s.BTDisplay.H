h22431
s 00000/00000/00000
d R 1.2 01/10/20 13:35:20 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTDisplay.H
c Name history : 1 0 src/widget/BTDisplay.H
e
s 00034/00000/00000
d D 1.1 01/10/20 13:35:19 bmc 1 0
c date and time created 01/10/20 13:35:19 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTDisplay.H                                         */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 11:30:39 1995                            */
/****************************************************************/


#ifndef BTDISPLAY_HEADER
#define BTDISPLAY_HEADER

#include "BTPalette.H"

class BTDisplay {

public:
  virtual void newPalette() = 0;
  virtual void handleEvents() = 0;
  virtual unsigned long addTimeout( unsigned long, void (*)(void *, unsigned long *),
				    void * ) = 0;
  virtual void removeTimeout( unsigned long ) = 0;
  virtual BTPalette *getPalette() { return palette_; };
  virtual void flush() {}
  ~BTDisplay() { delete palette_; }
  BTPalette *palette_;

protected:
  BTDisplay() : palette_(0) {}
};

extern BTDisplay *DISPLAY;

#endif
E 1
