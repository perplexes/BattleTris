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
