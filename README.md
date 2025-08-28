# BestellDesk  

BestellDesk is a Rust-based application designed to simplify the process of managing frequent food orders in environments such as offices or shared workspaces. It is particularly useful when a single person is responsible for collecting, organizing, and handling multiple orders from different restaurants. With BestellDesk, you can create and maintain menus, keep track of individual orders, and generate a clear payment overview for all participants.  

The application allows you to define and update menus for different restaurants manually. Once orders are collected, BestellDesk automatically generates an overview showing who ordered what and how much each person has to pay. This reduces errors, saves time, and keeps the ordering process transparent.  

## Features  

- Manage food orders for multiple people with ease  
- Create and maintain menus for different restaurants  
- Automatically generate payment overviews for participants  
- Database import and export exclusively with encryption  
- MongoDB Atlas support  
- Auto-update functionality for the application  
- GUI available on both Windows and Linux  
- Built with Rust for performance and stability  

## Technical Overview  

BestellDesk is written in Rust and provides a cross-platform graphical user interface. The data layer is powered by MongoDB, which enables structured and scalable storage for menus and orders. MongoDB Atlas is supported for cloud-based usage.  

Menus, orders, and payment summaries are represented as structured collections in the database. The database can only be imported or exported in encrypted form, ensuring that sensitive data remains secure. Rust’s strong type system ensures data integrity, while concurrency features guarantee smooth performance even with larger datasets.  

## Installation  

### Download Release  

You can download the latest release for your platform from the [Releases page](../../releases).  
After downloading, extract the archive and run the executable directly.  

### Build from Source  

1. Ensure you have [Rust](https://www.rust-lang.org/) installed.  
2. Set up [MongoDB Atlas](https://www.mongodb.com/atlas/database).  
3. Clone this repository:  
   ```bash
   git clone https://github.com/enzel-org/bestellapp.git
   cd bestelldesk
   ```
4. Build the project:
   ```bash
   cargo build --release
   ```
5. Run the application:
   ```bash
   cargo run
   ```
