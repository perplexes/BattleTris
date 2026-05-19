h51504
s 00093/00090/00046
d D 1.2 01/10/21 19:25:17 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:16 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTScrolledListWidget.C
c Name history : 1 0 src/widget/BTScrolledListWidget.C
e
s 00136/00000/00000
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
/*    FILE: BTScrolledListWidget.C                              */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 13:18:03 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTScrolledListWidget.H"

D 3
BTScrolledListWidget::BTScrolledListWidget(BTWidget *parent, char *const name,
                                           char **const list, int n)
E 3
I 3
BTScrolledListWidget::BTScrolledListWidget(BTWidget *parent, const char *name,
    const char **list, int n)
E 3
: BTWidget(parent), string_(0)
{
D 3
  XmStringTable strtab;
  Arg args[10];
  int i = 0, s;
E 3
I 3
	XmStringTable strtab;
	Arg args[10];
	int i = 0, s;
E 3

D 3
  XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
  XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
  XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
  XtSetArg(args[i], XmNlistSizePolicy, XmCONSTANT); i++;
  XtSetArg(args[i], XmNselectionPolicy, XmBROWSE_SELECT); i++;
  XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
  XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3
I 3
	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNlistSizePolicy, XmCONSTANT); i++;
	XtSetArg(args[i], XmNselectionPolicy, XmBROWSE_SELECT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3

D 3
  if(list) {
    strtab = (XmStringTable) XtMalloc(n * sizeof(XmString));
E 3
I 3
	if (list) {
		strtab = (XmStringTable) XtMalloc(n * sizeof(XmString));
E 3

D 3
    for(s = 0; s < n; s++)
      strtab[s] = XmStringCreateSimple(list[s]);
E 3
I 3
		for (s = 0; s < n; s++)
			strtab[s] = XmStringCreateSimple((char *)list[s]);
E 3

D 3
    XtSetArg(args[i], XmNvisibleItemCount, n); i++;
    XtSetArg(args[i], XmNitemCount, n); i++;
    XtSetArg(args[i], XmNitems, strtab); i++;
  }
E 3
I 3
		XtSetArg(args[i], XmNvisibleItemCount, n); i++;
		XtSetArg(args[i], XmNitemCount, n); i++;
		XtSetArg(args[i], XmNitems, strtab); i++;
  	}
E 3

D 3
  list_w_ =
    XmCreateScrolledList(parent ? parent->getWidget() : 0, name, args, i);
E 3
I 3
	list_w_ = XmCreateScrolledList(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);
E 3

D 3
  if(list) {
    for(s = 0; s < n; s++)
      XmStringFree(strtab[s]);
    XtFree((caddr_t) strtab);
  }
E 3
I 3
	if (list) {
		for (s = 0; s < n; s++)
			XmStringFree(strtab[s]);
		XtFree((caddr_t) strtab);
	}
E 3

D 3
  def_action_ = browse_sel_ = 0;
  XtAddCallback( list_w_, XmNdefaultActionCallback, defAction_CB, this );
  XtAddCallback( list_w_, XmNbrowseSelectionCallback, browseSel_CB, this );
E 3
I 3
	def_action_ = browse_sel_ = 0;
	XtAddCallback(list_w_, XmNdefaultActionCallback, defAction_CB, this);
	XtAddCallback(list_w_, XmNbrowseSelectionCallback, browseSel_CB, this);
E 3

D 3
  me_ = XtParent(list_w_); // Get the scrolled window widget
E 3
I 3
	me_ = XtParent(list_w_); // Get the scrolled window widget
E 3

D 3
  XtManageChild(list_w_);
E 3
I 3
	XtManageChild(list_w_);
E 3
}

D 3
BTScrolledListWidget::BTScrolledListWidget(BTWidget *parent, char *const name,
                                           char **const list, int n,
                                           Position x1, Position y1,
                                           Position x2, Position y2)
E 3
I 3
BTScrolledListWidget::BTScrolledListWidget(BTWidget *parent, const char *name,
   const char **list, int n, Position x1, Position y1, Position x2, Position y2)
E 3
: BTWidget(parent), string_(0)
{
D 3
  XmStringTable strtab;
  Arg args[18];
  int i = 0, s;
E 3
I 3
	XmStringTable strtab;
	Arg args[18];
	int i = 0, s;
E 3

D 3
  XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
  XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
  XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
  XtSetArg(args[i], XmNlistSizePolicy, XmCONSTANT); i++;
  XtSetArg(args[i], XmNselectionPolicy, XmBROWSE_SELECT); i++;
  XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
  XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3
I 3
	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNlistSizePolicy, XmCONSTANT); i++;
	XtSetArg(args[i], XmNselectionPolicy, XmBROWSE_SELECT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;
E 3


D 3
  XtSetArg(args[i], XmNtopAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNtopPosition, y1); i++;
  XtSetArg(args[i], XmNleftAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNleftPosition, x1); i++;
  XtSetArg(args[i], XmNrightAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNrightPosition, x2); i++;
  XtSetArg(args[i], XmNbottomAttachment, XmATTACH_POSITION); i++;
  XtSetArg(args[i], XmNbottomPosition, y2); i++;
E 3
I 3
	XtSetArg(args[i], XmNtopAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNtopPosition, y1); i++;
	XtSetArg(args[i], XmNleftAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNleftPosition, x1); i++;
	XtSetArg(args[i], XmNrightAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNrightPosition, x2); i++;
	XtSetArg(args[i], XmNbottomAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNbottomPosition, y2); i++;
E 3

D 3
  if(list) {
    strtab = (XmStringTable) XtMalloc(n * sizeof(XmString));
E 3
I 3
	if (list) {
		strtab = (XmStringTable)XtMalloc(n * sizeof(XmString));
E 3

D 3
    for(s = 0; s < n; s++)
      strtab[s] = XmStringCreateSimple(list[s]);
E 3
I 3
		for (s = 0; s < n; s++)
			strtab[s] = XmStringCreateSimple((char *)list[s]);
E 3

D 3
    XtSetArg(args[i], XmNvisibleItemCount, n); i++;
    XtSetArg(args[i], XmNitemCount, n); i++;
    XtSetArg(args[i], XmNitems, strtab); i++;
  }
E 3
I 3
		XtSetArg(args[i], XmNvisibleItemCount, n); i++;
		XtSetArg(args[i], XmNitemCount, n); i++;
		XtSetArg(args[i], XmNitems, strtab); i++;
	}
E 3

D 3
  list_w_ =
    XmCreateScrolledList(parent ? parent->getWidget() : 0, name, args, i);
E 3
I 3
	list_w_ = XmCreateScrolledList(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);
E 3

D 3
  if(list) {
    for(s = 0; s < n; s++)
      XmStringFree(strtab[s]);
    XtFree((caddr_t) strtab);
  }
E 3
I 3
	if (list) {
		for (s = 0; s < n; s++)
			XmStringFree(strtab[s]);
		XtFree((caddr_t) strtab);
	}
E 3

D 3
  def_action_ = browse_sel_ = 0;
  XtAddCallback( list_w_, XmNdefaultActionCallback, defAction_CB, this );
  XtAddCallback( list_w_, XmNbrowseSelectionCallback, browseSel_CB, this );
E 3
I 3
	def_action_ = browse_sel_ = 0;
	XtAddCallback(list_w_, XmNdefaultActionCallback, defAction_CB, this);
	XtAddCallback(list_w_, XmNbrowseSelectionCallback, browseSel_CB, this);
E 3

D 3
  me_ = XtParent(list_w_); // Get the scrolled window widget
  XtManageChild(list_w_);
E 3
I 3
	me_ = XtParent(list_w_); // Get the scrolled window widget
	XtManageChild(list_w_);
E 3
}

D 3
void BTScrolledListWidget::setList(char **const list, int n)
E 3
I 3
void
BTScrolledListWidget::setList(const char **list, int n)
E 3
{
D 3
  XmStringTable strtab;
  int i;
E 3
I 3
	XmStringTable strtab;
	int i;
E 3

D 3
  XmListDeselectAllItems(list_w_);
  XmListDeleteAllItems(list_w_);
E 3
I 3
	XmListDeselectAllItems(list_w_);
	XmListDeleteAllItems(list_w_);
E 3

D 3
  if(list == (char **) 0 || n == 0)
    return;
E 3
I 3
	if (list == NULL || n == 0)
		return;
E 3

D 3
  strtab = (XmStringTable) XtMalloc(n * sizeof(XmString));
E 3
I 3
	strtab = (XmStringTable)XtMalloc(n * sizeof(XmString));
E 3

D 3
  for(i = 0; i < n; i++)
    strtab[i] = XmStringCreateSimple(list[i]);
E 3
I 3
	for(i = 0; i < n; i++)
		strtab[i] = XmStringCreateSimple((char *)list[i]);
E 3

D 3
  XtVaSetValues(list_w_, XmNvisibleItemCount, n, XmNitemCount, n,
                XmNitems, strtab, NULL);
  XmListSelectItem(list_w_, strtab[0], False);
E 3
I 3
	XtVaSetValues(list_w_,
	    XmNvisibleItemCount, n,
	    XmNitemCount, n,
	    XmNitems, strtab,
	    NULL);
E 3

D 3
  for(i = 0; i < n; i++)
    XmStringFree(strtab[i]);
  XtFree((caddr_t) strtab);
E 3
I 3
	XmListSelectItem(list_w_, strtab[0], False);

	for (i = 0; i < n; i++)
		XmStringFree(strtab[i]);
	XtFree((caddr_t)strtab);
E 3
}
E 1
