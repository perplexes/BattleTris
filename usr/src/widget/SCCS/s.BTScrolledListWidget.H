h54018
s 00001/00000/00093
d D 1.3 01/10/22 22:56:08 bmc 4 3
c 1000016 ^C'ing causes seg fault under libumem
e
s 00061/00048/00032
d D 1.2 01/10/21 19:25:18 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:16 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTScrolledListWidget.H
c Name history : 1 0 src/widget/BTScrolledListWidget.H
e
s 00080/00000/00000
d D 1.1 01/10/20 13:35:15 bmc 1 0
c date and time created 01/10/20 13:35:15 by bmc
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
/*    FILE: BTScrolledList.H                                    */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 13:15:15 1994                            */
/****************************************************************/

#ifndef BTSCROLLEDLIST_H
#define BTSCROLLEDLIST_H

#include "BTConfig.H"

#include <Xm/List.h>

#include "BTWidget.H"

class BTScrolledListWidget : public BTWidget {
D 3

E 3
protected:
D 3
  Widget list_w_;
E 3
I 3
	Widget list_w_;
E 3

public:
D 3
  BTScrolledListWidget(BTWidget *parent, char *const name,
                       char **const list, int n);
  BTScrolledListWidget(BTWidget *parent, char *const name,
                       char **const list, int n,
		       Position x1, Position y1, Position x2, Position y2);
E 3
I 3
	BTScrolledListWidget(BTWidget *parent, const char *name,
	    const char **list, int n);
	BTScrolledListWidget(BTWidget *parent, const char *name,
	    const char **list, int n,
	    Position x1, Position y1, Position x2, Position y2);
E 3

D 3
  ~BTScrolledListWidget() {
    XtDestroyWidget(list_w_);
    if(def_action_)
      delete def_action_;
    if(browse_sel_)
      delete browse_sel_;
    if(string_)
      XtFree(string_);
  }
E 3
I 3
	~BTScrolledListWidget()
	{
		XtDestroyWidget(list_w_);
		if (def_action_)
			delete def_action_;
E 3

D 3
  Widget getListWidget() { return list_w_; }
  void setList(char **const list, int n);
E 3
I 3
		if (browse_sel_)
			delete browse_sel_;
E 3

D 3
  void selectPos( int pos, Boolean notify = 0 ) {
    XmListSelectPos( list_w_, pos, notify );
  }
E 3
I 3
		if (string_)
			XtFree(string_);
I 4
		me_ = NULL;
E 4
	}
E 3

D 3
  static void defAction_CB(Widget widget, XtPointer data, XtPointer call_data) {
    BTScrolledListWidget *t = (BTScrolledListWidget *) data;
    XmListCallbackStruct *cbs = (XmListCallbackStruct *) call_data;
    t->last_selection_ = cbs->item_position - 1;
    if(t->string_)
      XtFree(t->string_);
    XmStringGetLtoR(cbs->item, XmFONTLIST_DEFAULT_TAG, &t->string_);
    t->callback(t->def_action_);
  }
E 3
I 3
	Widget getListWidget() { return (list_w_); }
	void setList(const char **list, int n);
E 3

D 3
  virtual void addDefActionCallback(void (*cb)(BTWidget*, void*), void *data) {
    addCallback(def_action_, cb, data);
  }
E 3
I 3
	void
	selectPos(int pos, Boolean notify = 0)
	{
		XmListSelectPos( list_w_, pos, notify );
	}
E 3

D 3
  static void browseSel_CB(Widget widget, XtPointer data, XtPointer call_data) {
    BTScrolledListWidget *t = (BTScrolledListWidget *) data;
    XmListCallbackStruct *cbs = (XmListCallbackStruct *) call_data;
    t->last_selection_ = cbs->item_position - 1;
    if(t->string_)
      XtFree(t->string_);
    XmStringGetLtoR(cbs->item, XmFONTLIST_DEFAULT_TAG, &t->string_);
    t->callback(t->browse_sel_);
  }
E 3
I 3
	static void
	defAction_CB(Widget widget, XtPointer data, XtPointer call_data)
	{
		BTScrolledListWidget *t = (BTScrolledListWidget *)data;
		XmListCallbackStruct *cbs = (XmListCallbackStruct *)call_data;
		t->last_selection_ = cbs->item_position - 1;
		if (t->string_)
			XtFree(t->string_);
		XmStringGetLtoR(cbs->item, XmFONTLIST_DEFAULT_TAG, &t->string_);
		t->callback(t->def_action_);
	}
E 3

D 3
  virtual void addBrowseSelCallback(void (*cb)(BTWidget*, void*), void *data) {
    addCallback(browse_sel_, cb, data);
  }
E 3
I 3
	virtual void
	addDefActionCallback(void (*cb)(BTWidget*, void*), void *data)
	{
		addCallback(def_action_, cb, data);
	}
E 3

D 3
  ActivateCBStruct *def_action_, *browse_sel_;
  int last_selection_;
  char *string_;
E 3
I 3
	static void
	browseSel_CB(Widget widget, XtPointer data, XtPointer call_data)
	{
		BTScrolledListWidget *t = (BTScrolledListWidget *)data;
		XmListCallbackStruct *cbs = (XmListCallbackStruct *)call_data;
		t->last_selection_ = cbs->item_position - 1;
		if (t->string_)
			XtFree(t->string_);

		XmStringGetLtoR(cbs->item, XmFONTLIST_DEFAULT_TAG, &t->string_);
		t->callback(t->browse_sel_);
	}

	virtual void
	addBrowseSelCallback(void (*cb)(BTWidget*, void*), void *data)
	{
		addCallback(browse_sel_, cb, data);
	}

	ActivateCBStruct *def_action_, *browse_sel_;
	int last_selection_;
	char *string_;
E 3
};

#endif
E 1
