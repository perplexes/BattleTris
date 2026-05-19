h59383
s 00041/00028/00029
d D 1.2 01/10/21 19:25:16 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:15 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTPushButtonWidget.H
c Name history : 1 0 src/widget/BTPushButtonWidget.H
e
s 00057/00000/00000
d D 1.1 01/10/20 13:35:14 bmc 1 0
c date and time created 01/10/20 13:35:14 by bmc
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
/*    FILE: BTPushButtonWidget.H                                */
/*    ASGN: Images                                              */
/*    DATE: Tue Apr 19 20:04:17 1994                            */
/****************************************************************/

#ifndef BTPUSHBUTTONWIDGET_H
#define BTPUSHBUTTONWIDGET_H

#include "BTConfig.H"

#include <Xm/PushB.h>

#include "BTWidget.H"

class BTPushButtonWidget : public BTWidget {
public:
D 3
  BTPushButtonWidget(BTWidget *parent, char *const name, char *const title);
  BTPushButtonWidget(BTWidget *parent, char *const name, char *const title,
                     Position x1, Position y1, Position x2, Position y2);
  BTPushButtonWidget(BTWidget *parent, char *const name, char *const title, 
                     Dimension width, Dimension height);
  ~BTPushButtonWidget() {
    if ( activate_struct_ )
       delete activate_struct_;
  }
E 3
I 3
	BTPushButtonWidget(BTWidget *parent, const char *name,
	    const char *title);
	BTPushButtonWidget(BTWidget *parent, const char *name,
	    const char *title, Position x1, Position y1,
	    Position x2, Position y2);
	BTPushButtonWidget(BTWidget *parent, const char *name,
	    const char *title, Dimension width, Dimension height);
	~BTPushButtonWidget()
	{
		if (activate_struct_)
			delete activate_struct_;
	}
E 3

D 3
  void setLabel(char *const new_label);
E 3
I 3
	void setLabel(const char *const new_label);
E 3

D 3
  static void activate_CB(Widget widget, XtPointer data, XtPointer cbs) {
    BTPushButtonWidget *t = (BTPushButtonWidget *)data;
    t->callback( t->activate_struct_ );
  }
  virtual void addActivateCallback( void (*cb)(BTWidget *, void*), void *data ) {
    addCallback( activate_struct_, cb, data );
  }
E 3
I 3
	static void
	activate_CB(Widget widget, XtPointer data, XtPointer cbs)
	{
		BTPushButtonWidget *t = (BTPushButtonWidget *)data;
		t->callback(t->activate_struct_);
	}
E 3

D 3
  void alignLeft() {
    XtVaSetValues(me_, XmNalignment, XmALIGNMENT_BEGINNING, NULL);
  }
E 3
I 3
	virtual void
	addActivateCallback(void (*cb)(BTWidget *, void *), void *data)
	{
		addCallback( activate_struct_, cb, data );
	}
E 3

D 3
  void alignCenter() {
    XtVaSetValues(me_, XmNalignment, XmALIGNMENT_CENTER, NULL);
  }
E 3
I 3
	void
	alignLeft()
	{
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_BEGINNING, NULL);
	}

	void
	alignCenter()
	{
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_CENTER, NULL);
	}
E 3
  
D 3
  void alignRight() {
    XtVaSetValues(me_, XmNalignment, XmALIGNMENT_END, NULL);
  }
E 3
I 3
	void
	alignRight()
	{
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_END, NULL);
	}
E 3

protected:
D 3
  ActivateCBStruct *activate_struct_;

E 3
I 3
	ActivateCBStruct *activate_struct_;
E 3
};

#endif
E 1
