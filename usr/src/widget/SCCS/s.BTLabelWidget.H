h46743
s 00030/00029/00027
d D 1.2 01/10/21 19:25:15 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:14 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTLabelWidget.H
c Name history : 1 0 src/widget/BTLabelWidget.H
e
s 00056/00000/00000
d D 1.1 01/10/20 13:35:13 bmc 1 0
c date and time created 01/10/20 13:35:13 by bmc
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
/*    FILE: BTLabelWidget.H                                     */
/*    ASGN: Final                                               */
/*    DATE: Thu Apr 21 14:06:43 1994                            */
/****************************************************************/

#ifndef BTLABELWIDGET_H
#define BTLABELWIDGET_H

#include "BTConfig.H"

#include <Xm/Label.h>

#include "BTWidget.H"

class BTLabelWidget : public BTWidget {
public:
D 3
  BTLabelWidget(BTWidget *parent, char *const name, char *const title);
  BTLabelWidget(BTWidget *parent, char *const name, char *const title,
                Position x1, Position y1, Position x2, Position y2);
  BTLabelWidget(BTWidget *parent, char *const name, char *const title,
                Dimension width, Dimension height);
  ~BTLabelWidget() {
    if ( input_struct_ )
      delete input_struct_;
  }
E 3
I 3
	BTLabelWidget(BTWidget *parent, const char *name, const char *title);
	BTLabelWidget(BTWidget *parent, const char *name, const char *title,
	    Position x1, Position y1, Position x2, Position y2);
	BTLabelWidget(BTWidget *parent, const char *name, const char *title,
	    Dimension width, Dimension height);
	~BTLabelWidget()
	{
		if (input_struct_)
			delete input_struct_;
	}
E 3

D 3
  void setLabel(char * new_label);
E 3
I 3
	void setLabel(const char *new_label);
E 3

D 3
  void alignLeft() {
    XtVaSetValues(me_, XmNalignment, XmALIGNMENT_BEGINNING, NULL);
  }
E 3
I 3
	void alignLeft() {
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_BEGINNING, NULL);
	}
E 3

D 3
  void alignCenter() {
    XtVaSetValues(me_, XmNalignment, XmALIGNMENT_CENTER, NULL);
  }
E 3
I 3
	void alignCenter() {
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_CENTER, NULL);
	}
E 3
  
D 3
  void alignRight() {
    XtVaSetValues(me_, XmNalignment, XmALIGNMENT_END, NULL);
  }
E 3
I 3
	void alignRight() {
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_END, NULL);
	}
E 3

D 3
  static void input_CB(Widget widget, XtPointer data, XEvent *, Boolean *) {
    BTLabelWidget *t = (BTLabelWidget *) data;
    t->callback( t->input_struct_ );
  }
  virtual void addInputCallback( void (*cb)(BTWidget *, void*), void *data ) {
    addCallback( input_struct_, cb, data );
  }

  void setupStructs();
  ActivateCBStruct *input_struct_;
E 3
I 3
	static void input_CB(Widget widget, XtPointer data,
	    XEvent *, Boolean *)
	{
		BTLabelWidget *t = (BTLabelWidget *) data;
		t->callback(t->input_struct_);
	}
	virtual void addInputCallback(void (*cb)(BTWidget *, void*),
	    void *data) { addCallback( input_struct_, cb, data ); }
	void setupStructs();
	ActivateCBStruct *input_struct_;
E 3
};

#endif
E 1
