h48813
s 00005/00001/00039
d D 1.3 01/10/22 22:56:09 bmc 4 3
c 1000016 ^C'ing causes seg fault under libumem
e
s 00012/00013/00028
d D 1.2 01/10/21 19:25:18 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:17 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTScrolledTextWidget.H
c Name history : 1 0 src/widget/BTScrolledTextWidget.H
e
s 00041/00000/00000
d D 1.1 01/10/20 13:35:16 bmc 1 0
c date and time created 01/10/20 13:35:16 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTScrolledTextWidget.H                              */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 17:20:24 1994                            */
/****************************************************************/

#ifndef BTSCROLLEDTEXTWIDGET_H
#define BTSCROLLEDTEXTWIDGET_H

#include "BTConfig.H"

#include <Xm/Text.h>

#include "BTWidget.H"

D 3
enum {
  BTSCROLLEDTEXTWIDGET_DEFWIDTH = 44
};
E 3
I 3
#define BTSCROLLEDTEXTWIDGET_DEFWIDTH	44
E 3

class BTScrolledTextWidget : public BTWidget {
D 3

E 3
protected:
D 3
  Widget text_w_;
E 3
I 3
	Widget text_w_;
E 3

public:
D 3
  BTScrolledTextWidget(BTWidget *parent, char *const name, char *const text);
  BTScrolledTextWidget(BTWidget *parent, char *const name, char *const text,
                       Position x1, Position y1, Position x2, Position y2);
E 3
I 3
	BTScrolledTextWidget(BTWidget *parent, const char *name,
	    const char *text);
	BTScrolledTextWidget(BTWidget *parent, const char *name,
	    const char *text, Position x1, Position y1,
	    Position x2, Position y2);
E 3

D 3
  ~BTScrolledTextWidget() { XtDestroyWidget(text_w_); }
E 3
I 3
D 4
	~BTScrolledTextWidget() { XtDestroyWidget(text_w_); }
E 4
I 4
	~BTScrolledTextWidget()
	{
		XtDestroyWidget(text_w_);
		me_ = NULL;
	}
E 4
E 3

D 3
  void splitLines(char *text, int width);
  void setText(char *const text, Boolean split = True,
               int width = BTSCROLLEDTEXTWIDGET_DEFWIDTH);
E 3
I 3
	void splitLines(char *text, int width);
	void setText(const char *text, Boolean split = True,
	    int width = BTSCROLLEDTEXTWIDGET_DEFWIDTH);
E 3

D 3
  Widget getTextWidget() { return text_w_; }
E 3
I 3
	Widget getTextWidget() { return text_w_; }
E 3
};

#endif
E 1
